use futures::future::join_all;
use serde::{Deserialize, Serialize};
use worker::*;

/**
 * HackerNews item structure, can be used for both stories and comments
 * API endpoint: https://hacker-news.firebaseio.com/v0/item/${id}.json
 */
#[derive(Debug, Serialize, Deserialize)]
struct HNItem {
    id: u32,
    #[serde(rename = "type")]
    item_type: String, // "job", "story", "comment", "poll", "pollopt"

    deleted: Option<bool>,
    by: Option<String>,
    time: Option<u64>,
    text: Option<String>,
    dead: Option<bool>,
    parent: Option<u32>,
    poll: Option<u32>,
    kids: Option<Vec<u32>>,
    url: Option<String>,
    score: Option<u32>,
    title: Option<String>,
    parts: Option<Vec<u32>>,
    descendants: Option<u32>,
}

/**
 * Comment structure with nested replies
 */
#[derive(Debug, Serialize, Deserialize)]
struct CommentWithReplies {
    id: u32,

    by: Option<String>,
    time: Option<u64>,
    text: Option<String>,
    replies: Vec<CommentWithReplies>,
}

/**
 * Main response structure
 */
#[derive(Serialize)]
struct StoryWithComments {
    story: HNItem,
    comments: Vec<CommentWithReplies>,
}

#[event(fetch)]
async fn main(req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    let cors = Cors::new()
        .with_origins(vec!["*"])
        .with_methods(vec![Method::Get, Method::Post, Method::Options])
        .with_allowed_headers(vec!["Content-Type"]);

    let url = req.url()?;
    let path = url.path();

    if req.method() != Method::Get {
        return Response::error("Method Not Allowed", 405);
    }

    if path.starts_with("/api/story/") {
        let story_id = path.trim_start_matches("/api/story/");
        if !story_id.chars().all(char::is_numeric) {
            return Response::error("Invalid Story ID", 400);
        }

        let query_params = url.query_pairs();
        let mut max_depth = None;
        let mut limit = None;

        for (key, value) in query_params {
            if key == "depth" {
                if let Ok(depth) = value.parse::<u32>() {
                    max_depth = Some(depth)
                }
            } else if key == "limit" {
                if let Ok(lim) = value.parse::<usize>() {
                    limit = Some(lim)
                }
            }
        }

        match fetch_story_with_comments(story_id, max_depth, limit).await {
            Ok(story_with_comments) => {
                let json = serde_json::to_string(&story_with_comments)?;

                let mut response = Response::from_body(ResponseBody::Body(json.into_bytes()))?;
                response
                    .headers_mut()
                    .set("Content-type", "application/json")?;

                cors.apply_headers(response.headers_mut())?;

                Ok(response)
            }
            Err(e) => Response::error(format!("Error fetching data: {}", e), 500),
        }
    } else if path == "/" {
        // Return a simple HTML homepage
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Hacker News Comments API</title>
                <style>
                    body { font-family: sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
                    code { background: #f4f4f4; padding: 2px 4px; border-radius: 4px; }
                    table { border-collapse: collapse; width: 100%; margin: 20px 0; }
                    th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
                    th { background-color: #f4f4f4; }
                    tr:nth-child(even) { background-color: #f9f9f9; }
                </style>
            </head>
            <body>
                <h1>Hacker News Comments API</h1>
                <p>Basic usage: <code>/api/story/{story_id}</code></p>
                <p>Example: <a href="/api/story/36919310">/api/story/36919310</a></p>
                
                <h2>Query Parameters</h2>
                <table>
                    <tr>
                        <th>Parameter</th>
                        <th>Description</th>
                        <th>Default</th>
                        <th>Example</th>
                    </tr>
                    <tr>
                        <td>depth</td>
                        <td>Comment recursion depth, 0 means top-level comments only</td>
                        <td>10</td>
                        <td><a href="/api/story/36919310?depth=2">?depth=2</a></td>
                    </tr>
                    <tr>
                        <td>limit</td>
                        <td>Maximum number of comments per level</td>
                        <td>No limit</td>
                        <td><a href="/api/story/36919310?limit=5">?limit=5</a></td>
                    </tr>
                </table>
                
                <h2>Combined Query Examples</h2>
                <p>Limit depth and count: <a href="/api/story/36919310?depth=1&limit=10">/api/story/36919310?depth=1&limit=10</a></p>
                
                <p>Performance note: Worker now supports up to 5 minutes of processing time. For large stories, you can optimize performance by adjusting depth and limit parameters.</p>
            </body>
            </html>
            "#;

        let mut response = Response::from_body(ResponseBody::Body(html.into()))?;
        response.headers_mut().set("Content-Type", "text/html")?;

        Ok(response)
    } else {
        // Return 404 for unmatched paths
        Response::error("Not Found", 404)
    }
}

async fn fetch_hn_item(item_id: &str) -> Result<HNItem> {
    let url = format!(
        "https://hacker-news.firebaseio.com/v0/item/{}.json",
        item_id
    );
    let mut req = Request::new(&url, Method::Get)?;

    req.headers_mut()?.set("Accept", "application/json")?;

    let mut resp = Fetch::Request(req).send().await?;

    let status = resp.status_code();
    if !(200..=299).contains(&status) {
        return Err(Error::from(format!(
            "Failed to fetch item {}: HTTP {}",
            item_id, status
        )));
    }

    let body = resp.text().await?;
    let item: HNItem = serde_json::from_str(&body)?;

    Ok(item)
}

async fn fetch_comment_with_replies(
    comment_id: u32,
    max_depth: Option<u32>,
) -> Result<CommentWithReplies> {
    let comment = fetch_hn_item(&comment_id.to_string()).await?;

    let mut replies = Vec::new();
    let current_depth = max_depth.unwrap_or(10);

    if current_depth > 0 {
        if let Some(kids) = &comment.kids {
            let next_depth = Some(current_depth - 1);
            let fetches = kids
                .iter()
                .map(|&kid_id| fetch_comment_with_replies(kid_id, next_depth));

            let results = join_all(fetches).await;

            for result in results {
                if let Ok(reply) = result {
                    if reply.text.is_some() {
                        replies.push(reply)
                    }
                }
            }
        }
    }
    Ok(CommentWithReplies {
        id: comment_id,
        by: comment.by,
        time: comment.time,
        text: comment.text,
        replies,
    })
}

async fn fetch_story_with_comments(
    story_id: &str,
    max_depth: Option<u32>,
    limit: Option<usize>,
) -> Result<StoryWithComments> {
    let story = fetch_hn_item(story_id).await?;

    if story.item_type != "story" {
        return Err(Error::from("Provided ID is not a story"));
    }

    let mut comments = Vec::new();

    if let Some(kids) = &story.kids {
        let kid_ids = match limit {
            Some(lim) => kids.iter().take(lim).cloned().collect::<Vec<u32>>(),
            None => kids.clone(),
        };

        let fetches = kid_ids
            .iter()
            .map(|&kid_id| fetch_comment_with_replies(kid_id, max_depth));

        let results = join_all(fetches).await;

        for result in results {
            if let Ok(comment) = result {
                if comment.text.is_some() {
                    comments.push(comment);
                }
            }
        }
    }

    Ok(StoryWithComments { story, comments })
}

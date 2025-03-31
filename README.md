# Hacker News Comments API

A reliable API for fetching Hacker News stories with nested comments, built on Cloudflare Workers.

## API Endpoints

The API is publicly available at:

```
https://api.elowenluo.com/v1/hn/story/{story_id}
```

Where `{story_id}` is the ID of any Hacker News story.

## Features

- Fetch complete story details including title, URL, score, and author
- Retrieve nested comments with full threading
- Configure comment depth and limit for performance optimization
- CORS-enabled for browser usage
- Fast response time thanks to Cloudflare's global network

## Usage Examples

### Basic Story Fetch

```
https://api.elowenluo.com/v1/hn/story/43530889
```

### Query Parameters

| Parameter | Description | Default | Example |
|-----------|-------------|---------|---------|
| depth | Comment recursion depth (0 = top-level only) | 10 | `?depth=2` |
| limit | Maximum number of comments per level | No limit | `?limit=5` |

### Limiting Depth and Comment Count

```
https://api.elowenluo.com/v1/hn/story/43530889?depth=1&limit=10
```

## Response Format

The API returns JSON with the following structure:

```json
{
  "story": {
    "id": 43530889,
    "title": "Story Title",
    "url": "https://example.com",
    "by": "username",
    "time": 1679012345,
    "score": 123,
    "descendants": 45,
    // other story fields
  },
  "comments": [
    {
      "id": 123456,
      "by": "commenter",
      "time": 1679012400,
      "text": "Comment text here",
      "replies": [
        // Nested comment objects
      ]
    }
    // Additional comments
  ]
}
```

## Performance Notes

- The API supports up to 5 minutes of processing time
- For large stories with many comments, consider adjusting the `depth` and `limit` parameters
- The service is hosted on Cloudflare Workers for reliable global performance

## Implementation

Built with Rust and WebAssembly on Cloudflare Workers. The API fetches data directly from the official Hacker News Firebase API and transforms it into a more developer-friendly nested structure.

## License

MIT

## Contact

For issues or feature requests, please open an issue on the GitHub repository.
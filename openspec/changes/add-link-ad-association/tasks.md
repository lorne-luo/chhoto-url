## 1. Implementation
- [x] 1.1 Add optional `ad_id` foreign key to DbRow 
- [x] 1.2 Update link create/edit/list APIs to read/write ad association .
- [x] 1.3 Update admin UI forms/tables with ad selector .
- [x] 1.4 Add tests covering link creation/editing with/without ad, and invalid ad references.
- [x] 1.5 Implement ad deletion handler: when an ad is deleted, reset `DBRow.ad_id` to `None` for all links that reference the deleted ad ID.

## 2. Validation
- [x] 2.1 Run `openspec validate add-link-ad-association --strict`.


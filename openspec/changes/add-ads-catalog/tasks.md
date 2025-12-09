## 1. Implementation
- [ ] 1.1 Add `ads` table with columns: id (pk), unique name, image URL, ad link (not the short link), expiry, and `countdown_seconds` (default 5, range 0–30); enforce required fields but do not enforce link URL format.
- [ ] 1.2 Implement ads CRUD/list APIs with validation (unique name, required fields, countdown bounds 0–30) and expiry handling: show expired ads in CRUD lists, omit expired from selectable lists.
- [ ] 1.3 Add admin Ads tab UI for listing, creating, editing, deleting ads with validation feedback (including countdown input with default/validation), reusing existing short-link CRUD patterns to minimize code changes.
- [ ] 1.4 Add tests covering ads CRUD, validation (unique name, required fields, countdown defaults/bounds), and expiry filtering (visible in CRUD lists, omitted from selection).

## 2. Validation
- [ ] 2.1 Run `openspec validate add-ads-catalog --strict`.


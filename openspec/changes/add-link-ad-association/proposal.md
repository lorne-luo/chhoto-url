# Change: Link ads association and countdown settings

## Why
After creating a shared ads catalog, links need to reference an ad (optionally) in preparation for interstitial behavior.

## What Changes
- Extend links model/DB to store optional `ad_id` referencing catalog ads.
- Update link create/edit/list APIs and admin UI to set/view ad association.

## Impact
- Affected specs: `links` (updated capability).
- Affected code: `actix/src/database.rs` (link schema), `actix/src/services.rs` (link APIs), frontend admin UI (`resources/index.html`, `resources/static/script.js`), and related tests.


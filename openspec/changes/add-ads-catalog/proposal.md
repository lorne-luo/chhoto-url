# Change: Add ads catalog entity and admin tab

## Why
We need a reusable ads catalog so ads can be created once and reused across short links later. This stage adds the ads entity and admin UI without changing link behavior yet.

## What Changes
- Add `ads` table storing id, unique name, image URL, ad link (not the short link), expiry, and countdown_seconds (default 5, range 0–30 seconds, no link-format restriction).
- Add admin Ads tab to list/create/edit/delete ads with validation (required fields, unique name, countdown bounds, expiry handling) while still showing expired ads in CRUD lists.
- Serve ads data via backend CRUD/list endpoints; omit expired ads from selection lists intended for link association.

## Impact
- Affected specs: `ads` (new capability).
- Affected code: `actix/src/database.rs` (ads table), `actix/src/services.rs` (ads CRUD/list APIs), frontend admin UI (`resources/index.html`, `resources/static/script.js`), and tests for ads CRUD.


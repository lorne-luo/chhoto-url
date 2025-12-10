// SPDX-FileCopyrightText: 2023 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use log::{error, info};
use rusqlite::{fallible_iterator::FallibleIterator, Connection, ErrorCode};
use serde::Serialize;
use std::rc::Rc;

use crate::services::ChhotoError::{self, ClientError, ServerError};

// Struct for encoding a DB row
#[derive(Serialize)]
pub struct DBRow {
    shortlink: String,
    longlink: String,
    hits: i64,
    expiry_time: i64,
    ad_id: Option<i64>,
}

#[derive(Serialize)]
pub struct AdRow {
    pub id: i64,
    pub name: String,
    pub image_url: String,
    pub ad_link: String,
    pub expiry_time: i64,
    pub countdown_seconds: i64,
}

// Find a single URL for /api/expand
pub fn find_url(
    shortlink: &str,
    db: &Connection,
) -> Result<(String, i64, i64, Option<i64>), ChhotoError> {
    // Long link, hits, expiry time, ad_id
    let now = chrono::Utc::now().timestamp();
    let query = "SELECT long_url, hits, expiry_time, ad_id FROM urls
                 WHERE short_url = ?1 
                 AND (expiry_time = 0 OR expiry_time > ?2)";
    let Ok(mut statement) = db.prepare_cached(query) else {
        error!("Error preparing SQL statement for find_url.");
        return Err(ServerError);
    };
    statement
        .query_row((shortlink, now), |row| {
            Ok((
                row.get("long_url")?,
                row.get("hits")?,
                row.get("expiry_time")?,
                row.get("ad_id")?,
            ))
        })
        .map_err(|_| ChhotoError::ClientError {
            reason: "The shortlink does not exist on the server!".to_string(),
        })
}

// Get all URLs in DB
pub fn getall(
    db: &Connection,
    page_after: Option<&str>,
    page_no: Option<i64>,
    page_size: Option<i64>,
) -> Rc<[DBRow]> {
    let now = chrono::Utc::now().timestamp();
    let query = if page_after.is_some() {
        "SELECT short_url, long_url, hits, expiry_time, ad_id FROM (
            SELECT t.id, t.short_url, t.long_url, t.hits, t.expiry_time, t.ad_id FROM urls AS t 
            JOIN urls AS u ON u.short_url = ?1 
            WHERE t.id < u.id AND (t.expiry_time = 0 OR t.expiry_time > ?2) 
            ORDER BY t.id DESC LIMIT ?3
         ) ORDER BY id ASC"
    } else if page_no.is_some() {
        "SELECT short_url, long_url, hits, expiry_time, ad_id FROM (
            SELECT id, short_url, long_url, hits, expiry_time, ad_id FROM urls 
            WHERE expiry_time= 0 OR expiry_time > ?1 
            ORDER BY id DESC LIMIT ?2 OFFSET ?3
         ) ORDER BY id ASC"
    } else if page_size.is_some() {
        "SELECT short_url, long_url, hits, expiry_time, ad_id FROM (
            SELECT id, short_url, long_url, hits, expiry_time, ad_id FROM urls
            WHERE expiry_time = 0 OR expiry_time > ?1 
            ORDER BY id DESC LIMIT ?2
         ) ORDER BY id ASC"
    } else {
        "SELECT short_url, long_url, hits, expiry_time, ad_id
         FROM urls WHERE expiry_time = 0 OR expiry_time > ?1 
         ORDER BY id ASC"
    };
    let Ok(mut statement) = db.prepare_cached(query) else {
        error!("Error preparing SQL statement for getall.");
        return [].into();
    };

    let raw_data = if let Some(pos) = page_after {
        let size = page_size.unwrap_or(10);
        statement.query((pos, now, size))
    } else if let Some(num) = page_no {
        let size = page_size.unwrap_or(10);
        statement.query((now, size, (num - 1) * size))
    } else if let Some(size) = page_size {
        statement.query((now, size))
    } else {
        statement.query([now])
    };

    let Ok(data) = raw_data else {
        error!("Error running SQL statement for getall: {query}");
        return [].into();
    };

    let links: Rc<[DBRow]> = data
        .map(|row| {
            Ok(DBRow {
                shortlink: row.get("short_url")?,
                longlink: row.get("long_url")?,
                hits: row.get("hits")?,
                expiry_time: row.get("expiry_time")?,
                ad_id: row.get("ad_id")?,
            })
        })
        .collect()
        .unwrap_or_else(|err| {
            error!("Error processing fetched rows: {err}");
            [].into()
        });

    links
}

// Add a hit when site is visited during link resolution
pub fn find_and_add_hit(shortlink: &str, db: &Connection) -> Result<String, ()> {
    let now = chrono::Utc::now().timestamp();
    let Ok(mut statement) = db.prepare_cached(
        "UPDATE urls 
             SET hits = hits + 1 
             WHERE short_url = ?1 AND (expiry_time = 0 OR expiry_time > ?2)
             RETURNING long_url",
    ) else {
        error!("Error preparing SQL statement for add_hit.");
        return Err(());
    };
    statement
        .query_one((shortlink, now), |row| row.get("long_url"))
        .map_err(|_| ())
}

// Insert a new link
pub fn add_link(
    shortlink: &str,
    longlink: &str,
    expiry_delay: i64,
    ad_id: Option<i64>,
    db: &Connection,
) -> Result<i64, ChhotoError> {
    let now = chrono::Utc::now().timestamp();
    let expiry_time = if expiry_delay == 0 {
        0
    } else {
        now + expiry_delay
    };

    let Ok(mut statement) = db.prepare_cached(
        "INSERT INTO urls
             (long_url, short_url, hits, expiry_time, ad_id)
             VALUES (?1, ?2, 0, ?3, ?4)
             ON CONFLICT(short_url) DO UPDATE 
             SET long_url = ?1, hits = 0, expiry_time = ?3, ad_id = ?4
             WHERE short_url = ?2 AND expiry_time <= ?5 AND expiry_time > 0",
    ) else {
        error!("Error preparing SQL statement for add_link.");
        return Err(ServerError);
    };
    match statement.execute((longlink, shortlink, expiry_time, ad_id, now)) {
        Ok(1) => Ok(expiry_time),
        Ok(_) => Err(ClientError {
            reason: "Short URL is already in use!".to_string(),
        }),
        Err(e) => {
            error!("There was some error while adding the link ({shortlink}, {longlink}, {expiry_delay}): {e}");
            Err(ServerError)
        }
    }
}

// Edit an existing link
pub fn edit_link(
    shortlink: &str,
    longlink: &str,
    reset_hits: bool,
    ad_id: Option<Option<i64>>,
    db: &Connection,
) -> Result<usize, ()> {
    let now = chrono::Utc::now().timestamp();
    let result = match (reset_hits, ad_id) {
        (true, Some(ad)) => {
            let Ok(mut statement) = db.prepare_cached(
                "UPDATE urls 
                 SET long_url = ?1, hits = 0, ad_id = ?2
                 WHERE short_url = ?3 AND (expiry_time = 0 OR expiry_time > ?4)",
            ) else {
                error!("Error preparing SQL statement for edit_link with ad update.");
                return Err(());
            };
            statement.execute((longlink, ad, shortlink, now))
        }
        (false, Some(ad)) => {
            let Ok(mut statement) = db.prepare_cached(
                "UPDATE urls 
                 SET long_url = ?1, ad_id = ?2
                 WHERE short_url = ?3 AND (expiry_time = 0 OR expiry_time > ?4)",
            ) else {
                error!("Error preparing SQL statement for edit_link with ad update.");
                return Err(());
            };
            statement.execute((longlink, ad, shortlink, now))
        }
        (true, None) => {
            let Ok(mut statement) = db.prepare_cached(
                "UPDATE urls 
                 SET long_url = ?1, hits = 0 
                 WHERE short_url = ?2 AND (expiry_time = 0 OR expiry_time > ?3)",
            ) else {
                error!("Error preparing SQL statement for edit_link.");
                return Err(());
            };

            statement.execute((longlink, shortlink, now))
        }
        (false, None) => {
            let Ok(mut statement) = db.prepare_cached(
                "UPDATE urls 
                 SET long_url = ?1 
                 WHERE short_url = ?2 AND (expiry_time = 0 OR expiry_time > ?3)",
            ) else {
                error!("Error preparing SQL statement for edit_link.");
                return Err(());
            };

            statement.execute((longlink, shortlink, now))
        }
    };

    result
        .inspect_err(|err| {
            error!(
                "Got an error while editing link ({shortlink}, {longlink}, {reset_hits}): {err}"
            );
        })
        .map_err(|_| ())
}

// Clean expired links
pub fn cleanup(db: &Connection, use_wal_mode: bool) {
    let now = chrono::Utc::now().timestamp();
    info!("Starting database cleanup.");

    let mut statement = db
        .prepare_cached("DELETE FROM urls WHERE ?1 >= expiry_time AND expiry_time > 0")
        .expect("Error preparing SQL statement for cleanup.");
    statement
        .execute([now])
        .inspect(|&u| match u {
            0 => (),
            1 => info!("1 link was deleted."),
            _ => info!("{u} links were deleted."),
        })
        .expect("Error cleaning expired links.");

    if use_wal_mode {
        let mut pragma_statement = db
            .prepare_cached("PRAGMA wal_checkpoint(TRUNCATE)")
            .expect("Error preparing SQL statement for pragma: wal_checkpoint.");
        pragma_statement
            .query_one([], |row| row.get::<usize, isize>(1))
            .ok()
            .filter(|&v| v != -1)
            .expect("Unable to create WAL checkpoint.");
    }
    let mut pragma_statement = db
        .prepare_cached("PRAGMA optimize")
        .expect("Error preparing SQL statement for pragma: optimize.");
    pragma_statement
        .execute([])
        .expect("Unable to optimize database.");
    info!("Optimized database.")
}

// Delete an existing link
pub fn delete_link(shortlink: &str, db: &Connection) -> Result<(), ChhotoError> {
    let Ok(mut statement) = db.prepare_cached("DELETE FROM urls WHERE short_url = ?1") else {
        error!("Error preparing SQL statement for delete_link.");
        return Err(ServerError);
    };
    match statement.execute([shortlink]) {
        Ok(delta) if delta > 0 => Ok(()),
        _ => Err(ClientError {
            reason: "The shortlink was not found, and could not be deleted.".to_string(),
        }),
    }
}

pub fn list_ads(db: &Connection) -> Rc<[AdRow]> {
    let Ok(mut statement) = db.prepare_cached(
        "SELECT id, name, image_url, ad_link, expiry_time, countdown_seconds
         FROM ads
         ORDER BY id ASC",
    ) else {
        error!("Error preparing SQL statement for list_ads.");
        return [].into();
    };

    let Ok(data) = statement.query([]) else {
        error!("Error running SQL statement for list_ads.");
        return [].into();
    };

    data.map(|row| {
        Ok(AdRow {
            id: row.get("id")?,
            name: row.get("name")?,
            image_url: row.get("image_url")?,
            ad_link: row.get("ad_link")?,
            expiry_time: row.get("expiry_time")?,
            countdown_seconds: row.get("countdown_seconds")?,
        })
    })
    .collect()
    .unwrap_or_else(|err| {
        error!("Error processing fetched ads rows: {err}");
        [].into()
    })
}

pub fn list_active_ads(db: &Connection) -> Rc<[AdRow]> {
    let now = chrono::Utc::now().timestamp();
    let Ok(mut statement) = db.prepare_cached(
        "SELECT id, name, image_url, ad_link, expiry_time, countdown_seconds
         FROM ads
         WHERE expiry_time = 0 OR expiry_time > ?1
         ORDER BY id ASC",
    ) else {
        error!("Error preparing SQL statement for list_active_ads.");
        return [].into();
    };

    let Ok(data) = statement.query([now]) else {
        error!("Error running SQL statement for list_active_ads.");
        return [].into();
    };

    data.map(|row| {
        Ok(AdRow {
            id: row.get("id")?,
            name: row.get("name")?,
            image_url: row.get("image_url")?,
            ad_link: row.get("ad_link")?,
            expiry_time: row.get("expiry_time")?,
            countdown_seconds: row.get("countdown_seconds")?,
        })
    })
    .collect()
    .unwrap_or_else(|err| {
        error!("Error processing fetched active ads rows: {err}");
        [].into()
    })
}

pub fn ad_exists(id: i64, db: &Connection) -> bool {
    let Ok(mut statement) = db.prepare_cached("SELECT 1 FROM ads WHERE id = ?1 LIMIT 1") else {
        error!("Error preparing SQL statement for ad existence check.");
        return false;
    };

    statement
        .exists([id])
        .map_err(|err| {
            error!("Error checking ad existence for {id}: {err}");
        })
        .unwrap_or(false)
}

pub fn clear_ad_references(ad_id: i64, db: &Connection) -> Result<usize, ChhotoError> {
    let Ok(mut statement) = db.prepare_cached("UPDATE urls SET ad_id = NULL WHERE ad_id = ?1")
    else {
        error!("Error preparing SQL statement for clearing ad references.");
        return Err(ServerError);
    };

    statement
        .execute([ad_id])
        .inspect_err(|err| {
            error!("Error clearing ad references for {ad_id}: {err}");
        })
        .map_err(|_| ServerError)
}

pub fn insert_ad(
    name: &str,
    image_url: &str,
    ad_link: &str,
    expiry_time: i64,
    countdown_seconds: i64,
    db: &Connection,
) -> Result<AdRow, ChhotoError> {
    let Ok(mut statement) = db.prepare_cached(
        "INSERT INTO ads (name, image_url, ad_link, expiry_time, countdown_seconds)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(name) DO NOTHING",
    ) else {
        error!("Error preparing SQL statement for insert_ad.");
        return Err(ServerError);
    };

    match statement.execute((name, image_url, ad_link, expiry_time, countdown_seconds)) {
        Ok(1) => {
            let id = db.last_insert_rowid();
            Ok(AdRow {
                id,
                name: name.to_string(),
                image_url: image_url.to_string(),
                ad_link: ad_link.to_string(),
                expiry_time,
                countdown_seconds,
            })
        }
        Ok(_) => Err(ClientError {
            reason: "Ad name is already in use!".to_string(),
        }),
        Err(err) => {
            error!("There was some error while adding ad ({name}, {image_url}, {ad_link}): {err}");
            Err(ServerError)
        }
    }
}

pub fn update_ad(
    id: i64,
    name: &str,
    image_url: &str,
    ad_link: &str,
    expiry_time: i64,
    countdown_seconds: i64,
    db: &Connection,
) -> Result<AdRow, ChhotoError> {
    let Ok(mut statement) = db.prepare_cached(
        "UPDATE ads
         SET name = ?1,
             image_url = ?2,
             ad_link = ?3,
             expiry_time = ?4,
             countdown_seconds = ?5
         WHERE id = ?6",
    ) else {
        error!("Error preparing SQL statement for update_ad.");
        return Err(ServerError);
    };

    match statement.execute((name, image_url, ad_link, expiry_time, countdown_seconds, id)) {
        Ok(1) => Ok(AdRow {
            id,
            name: name.to_string(),
            image_url: image_url.to_string(),
            ad_link: ad_link.to_string(),
            expiry_time,
            countdown_seconds,
        }),
        Ok(_) => Err(ClientError {
            reason: "The ad was not found, and could not be edited.".to_string(),
        }),
        Err(err) => {
            if err.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
                return Err(ClientError {
                    reason: "Ad name is already in use!".to_string(),
                });
            }
            error!(
                "There was some error while updating ad ({name}, {image_url}, {ad_link}): {err}"
            );
            Err(ServerError)
        }
    }
}

pub fn delete_ad(id: i64, db: &Connection) -> Result<(), ChhotoError> {
    // Reset ad references on links before deleting the ad itself
    clear_ad_references(id, db)?;

    let Ok(mut statement) = db.prepare_cached("DELETE FROM ads WHERE id = ?1") else {
        error!("Error preparing SQL statement for delete_ad.");
        return Err(ServerError);
    };
    match statement.execute([id]) {
        Ok(delta) if delta > 0 => Ok(()),
        Ok(_) => Err(ClientError {
            reason: "The ad was not found, and could not be deleted.".to_string(),
        }),
        Err(err) => {
            error!("There was some error while deleting ad ({id}): {err}");
            Err(ServerError)
        }
    }
}

pub fn open_db(path: &str, use_wal_mode: bool, ensure_acid: bool) -> Connection {
    // Set current user_version. Should be incremented on change of schema.
    let user_version = 3;

    let db = Connection::open(path).expect("Unable to open database!");

    // It would be 0 if table does not exist, and 1 if it does
    let table_exists: usize = db
        .query_row_and_then(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'urls'",
            [],
            |row| row.get(0),
        )
        .expect("Error querying existence of table.");

    // Create table if it doesn't exist
    db.execute(
        "CREATE TABLE IF NOT EXISTS urls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            long_url TEXT NOT NULL,
            short_url TEXT NOT NULL,
            hits INTEGER NOT NULL,
            expiry_time INTEGER NOT NULL DEFAULT 0,
            ad_id INTEGER
         )",
        // expiry_time is added later during migration 1
        [],
    )
    .expect("Unable to initialize empty database.");

    // Create index on short_url for faster lookups
    db.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_short_url ON urls (short_url)",
        [],
    )
    .expect("Unable to create index on short_url.");

    let current_user_version: u32 = if table_exists == 0 {
        // It would mean that the table is newly created i.e. has the desired schema
        user_version
    } else {
        db.query_row_and_then("SELECT user_version FROM pragma_user_version", [], |row| {
            row.get(0)
        })
        .unwrap_or_default()
    };

    // Migration 1: Add expiry_time, introduced in 6.0.0
    if current_user_version < 1 {
        db.execute(
            "ALTER TABLE urls ADD COLUMN expiry_time INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .expect("Unable to apply migration 1.");
    }

    // Migration 2: Add ad_id column to urls to store optional ad references
    if current_user_version < 3 {
        if let Err(err) = db.execute("ALTER TABLE urls ADD COLUMN ad_id INTEGER", []) {
            if !err.to_string().contains("duplicate column name") {
                panic!("Unable to apply migration 2: {err}");
            }
        }
        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_urls_ad_id ON urls (ad_id)",
            [],
        )
        .expect("Unable to create index on urls ad_id.");
    }

    // Create index on expiry_time for faster lookups
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_expiry_time ON urls (expiry_time)",
        [],
    )
    .expect("Unable to create index on expiry_time.");

    db.execute(
        "CREATE TABLE IF NOT EXISTS ads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            image_url TEXT NOT NULL,
            ad_link TEXT NOT NULL,
            expiry_time INTEGER NOT NULL DEFAULT 0,
            countdown_seconds INTEGER NOT NULL DEFAULT 5,
            CONSTRAINT ads_name_unique UNIQUE (name)
        )",
        [],
    )
    .expect("Unable to initialize ads table.");

    db.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_ads_name ON ads (name)",
        [],
    )
    .expect("Unable to create index on ads name.");

    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_ads_expiry_time ON ads (expiry_time)",
        [],
    )
    .expect("Unable to create index on ads expiry_time.");

    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_urls_ad_id ON urls (ad_id)",
        [],
    )
    .expect("Unable to create index on urls ad_id.");

    // Set the user version
    db.pragma_update(None, "user_version", user_version)
        .expect("Unable to set pragma: user_version.");
    // Set WAL mode if specified
    let (journal_mode, synchronous) = match (use_wal_mode, ensure_acid) {
        (true, false) => ("WAL", "NORMAL"),
        (true, true) => ("WAL", "FULL"),
        (false, false) => ("DELETE", "FULL"),
        (false, true) => ("DELETE", "EXTRA"),
    };
    db.pragma_update(None, "journal_mode", journal_mode)
        .expect("Unable to set pragma: journal_mode.");
    db.pragma_update(None, "synchronous", synchronous)
        .expect("Unable to set pragma: synchronous.");
    // Set some further optimizations and run vacuum
    db.pragma_update(None, "temp_store", "memory")
        .expect("Unable to set pragma: temp_store.");
    db.pragma_update(None, "journal_size_limit", "8388608")
        .expect("Unable to set pragma: journal_size_limit.");
    db.pragma_update(None, "mmap_size", "16777216")
        .expect("Unable to set pragma: mmap_size.");
    db.execute("VACUUM", []).expect("Unable to vacuum database");
    db.execute("PRAGMA optimize=0x10002", [])
        .expect("Error running pragma optimize.");

    db
}

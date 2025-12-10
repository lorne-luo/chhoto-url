// SPDX-FileCopyrightText: 2023 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use chrono::Utc;
use log::error;
use nanoid::nanoid;
use rand::seq::IndexedRandom;
use regex::Regex;
use rusqlite::Connection;
use serde::{de::Deserializer, Deserialize};

use crate::{
    config::Config,
    database::{self, AdRow},
    services::{
        ChhotoError::{self, ClientError, ServerError},
        GetReqParams,
    },
};

// Struct for reading link pairs sent during API call for new link
#[derive(Deserialize)]
struct NewURLRequest {
    #[serde(default)]
    shortlink: String,
    longlink: String,
    #[serde(default)]
    expiry_delay: i64,
    #[serde(default)]
    ad_id: Option<i64>,
}

// Struct for reading link pairs sent during API call for editing link
#[derive(Deserialize)]
struct EditURLRequest {
    shortlink: String,
    longlink: String,
    reset_hits: bool,
    #[serde(default, deserialize_with = "deserialize_optional_ad_id")]
    ad_id: Option<Option<i64>>,
}

fn deserialize_optional_ad_id<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(Option::<i64>::deserialize(deserializer)?))
}

#[derive(Deserialize)]
struct AdRequest {
    name: String,
    image_url: String,
    ad_link: String,
    #[serde(default)]
    expiry_delay: i64,
    #[serde(default)]
    countdown_seconds: Option<i64>,
}

// Only have a-z, 0-9, - and _ as valid characters in a shortlink
fn is_link_valid(link: &str, allow_capital_letters: bool) -> bool {
    let re = if allow_capital_letters {
        Regex::new("^[A-Za-z0-9-_]+$").expect("Regex generation failed.")
    } else {
        Regex::new("^[a-z0-9-_]+$").expect("Regex generation failed.")
    };
    re.is_match(link)
}

// Request the DB for all URLs
pub fn getall(db: &Connection, params: GetReqParams) -> String {
    let page_after = params.page_after.filter(|s| !s.is_empty());
    let page_no = params.page_no.filter(|&n| n > 0);
    let page_size = params.page_size.filter(|&n| n > 0);
    let links = database::getall(db, page_after.as_deref(), page_no, page_size);
    serde_json::to_string(&links).expect("Failure during creation of json from db.")
}

// Make checks and then request the DB to add a new URL entry
pub fn add_link(
    req: &str,
    db: &Connection,
    config: &Config,
    using_public_mode: bool,
) -> Result<(String, i64, Option<i64>), ChhotoError> {
    // Ok : shortlink, expiry_time
    let mut chunks: NewURLRequest;
    if let Ok(json) = serde_json::from_str(req) {
        chunks = json;
    } else {
        return Err(ClientError {
            reason: "Invalid request!".to_string(),
        });
    }

    let style = &config.slug_style;
    let len = config.slug_length;
    let allow_capital_letters = config.allow_capital_letters;
    let shortlink_provided = if chunks.shortlink.is_empty() {
        chunks.shortlink = gen_link(style, len, allow_capital_letters);
        false
    } else {
        true
    };

    // In public mode, set automatic expiry delay
    if using_public_mode && config.public_mode_expiry_delay > 0 {
        if chunks.expiry_delay == 0 {
            chunks.expiry_delay = config.public_mode_expiry_delay;
        } else {
            chunks.expiry_delay = chunks.expiry_delay.min(config.public_mode_expiry_delay);
        }
    }

    // Allow max delay of 5 years
    chunks.expiry_delay = chunks.expiry_delay.min(157784760);
    chunks.expiry_delay = chunks.expiry_delay.max(0);

    if !shortlink_provided || is_link_valid(chunks.shortlink.as_str(), allow_capital_letters) {
        if let Some(ad_id) = chunks.ad_id {
            if ad_id <= 0 || !database::ad_exists(ad_id, db) {
                return Err(ClientError {
                    reason: "Invalid ad id.".to_string(),
                });
            }
        }

        match database::add_link(
            &chunks.shortlink,
            &chunks.longlink,
            chunks.expiry_delay,
            chunks.ad_id,
            db,
        ) {
            Ok(expiry_time) => Ok((chunks.shortlink, expiry_time, chunks.ad_id)),
            Err(ClientError { reason }) => {
                if shortlink_provided {
                    Err(ClientError { reason })
                } else {
                    // Optionally, retry with a longer slug length
                    let retry_len = if config.slug_style == "UID" && config.try_longer_slug {
                        len + 4
                    } else {
                        len
                    };
                    chunks.shortlink = gen_link(style, retry_len, allow_capital_letters);
                    match database::add_link(
                        &chunks.shortlink,
                        &chunks.longlink,
                        chunks.expiry_delay,
                        chunks.ad_id,
                        db,
                    ) {
                        Ok(expiry_time) => Ok((chunks.shortlink, expiry_time, chunks.ad_id)),
                        Err(_) => {
                            error!("Something went wrong while adding a generated link.");
                            Err(ServerError)
                        }
                    }
                }
            }
            Err(ServerError) => Err(ServerError),
        }
    } else {
        Err(ClientError {
            reason: "Short URL is not valid!".to_string(),
        })
    }
}

// Make checks and then request the DB to edit an URL entry
pub fn edit_link(req: &str, db: &Connection, config: &Config) -> Result<(), ChhotoError> {
    let chunks: EditURLRequest;
    if let Ok(json) = serde_json::from_str(req) {
        chunks = json;
    } else {
        return Err(ClientError {
            reason: "Malformed request!".to_string(),
        });
    }
    if !is_link_valid(&chunks.shortlink, config.allow_capital_letters) {
        return Err(ClientError {
            reason: "Invalid shortlink!".to_string(),
        });
    }
    if let Some(Some(ad_id)) = chunks.ad_id {
        if ad_id <= 0 || !database::ad_exists(ad_id, db) {
            return Err(ClientError {
                reason: "Invalid ad id.".to_string(),
            });
        }
    }
    let result = database::edit_link(
        &chunks.shortlink,
        &chunks.longlink,
        chunks.reset_hits,
        chunks.ad_id,
        db,
    );
    match result {
        // Zero rows returned means no updates
        Ok(0) => Err(ClientError {
            reason: "The shortlink was not found, and could not be edited.".to_string(),
        }),
        Ok(_) => Ok(()),
        Err(()) => Err(ServerError),
    }
}
// Check if link, and request DB to delete it if exists
pub fn delete_link(
    shortlink: &str,
    db: &Connection,
    allow_capital_letters: bool,
) -> Result<(), ChhotoError> {
    if is_link_valid(shortlink, allow_capital_letters) {
        database::delete_link(shortlink, db)
    } else {
        Err(ClientError {
            reason: "The shortlink is invalid.".to_string(),
        })
    }
}

pub fn create_ad(req: &str, db: &Connection) -> Result<AdRow, ChhotoError> {
    let chunks: AdRequest = serde_json::from_str(req).map_err(|_| ClientError {
        reason: "Invalid request!".to_string(),
    })?;

    let validated = validate_ad_payload(chunks)?;
    database::insert_ad(
        &validated.name,
        &validated.image_url,
        &validated.ad_link,
        validated.expiry_time,
        validated.countdown_seconds,
        db,
    )
}

pub fn edit_ad(id: i64, req: &str, db: &Connection) -> Result<AdRow, ChhotoError> {
    if id <= 0 {
        return Err(ClientError {
            reason: "Invalid ad id.".to_string(),
        });
    }
    let chunks: AdRequest = serde_json::from_str(req).map_err(|_| ClientError {
        reason: "Invalid request!".to_string(),
    })?;

    let validated = validate_ad_payload(chunks)?;
    database::update_ad(
        id,
        &validated.name,
        &validated.image_url,
        &validated.ad_link,
        validated.expiry_time,
        validated.countdown_seconds,
        db,
    )
}

struct ValidatedAd {
    name: String,
    image_url: String,
    ad_link: String,
    expiry_time: i64,
    countdown_seconds: i64,
}

fn validate_ad_payload(chunks: AdRequest) -> Result<ValidatedAd, ChhotoError> {
    let name = chunks.name.trim().to_string();
    if name.is_empty() {
        return Err(ClientError {
            reason: "Ad name is required.".to_string(),
        });
    }
    let image_url = chunks.image_url.trim().to_string();
    if image_url.is_empty() {
        return Err(ClientError {
            reason: "Image URL is required.".to_string(),
        });
    }
    let ad_link = chunks.ad_link.trim().to_string();
    if ad_link.is_empty() {
        return Err(ClientError {
            reason: "Ad link is required.".to_string(),
        });
    }

    let mut expiry_delay = chunks.expiry_delay;
    expiry_delay = expiry_delay.max(0);
    // keep max at ~5 years for parity with links
    expiry_delay = expiry_delay.min(157_784_760);
    let expiry_time = if expiry_delay == 0 {
        0
    } else {
        Utc::now().timestamp() + expiry_delay
    };

    let countdown_seconds = chunks.countdown_seconds.unwrap_or(5);
    if !(0..=30).contains(&countdown_seconds) {
        return Err(ClientError {
            reason: "Countdown must be between 0 and 30 seconds.".to_string(),
        });
    }

    Ok(ValidatedAd {
        name,
        image_url,
        ad_link,
        expiry_time,
        countdown_seconds,
    })
}

// Generate a random link using either adjective-name pair (default) of a slug or a-z, 0-9
fn gen_link(style: &str, len: usize, allow_capital_letters: bool) -> String {
    #[rustfmt::skip]
    static ADJECTIVES: [&str; 108] = ["admiring", "adoring", "affectionate", "agitated", "amazing", "angry", "awesome", "beautiful", 
		"blissful", "bold", "boring", "brave", "busy", "charming", "clever", "compassionate", "competent", "condescending", "confident", "cool", 
		"cranky", "crazy", "dazzling", "determined", "distracted", "dreamy", "eager", "ecstatic", "elastic", "elated", "elegant", "eloquent", "epic", 
		"exciting", "fervent", "festive", "flamboyant", "focused", "friendly", "frosty", "funny", "gallant", "gifted", "goofy", "gracious", 
		"great", "happy", "hardcore", "heuristic", "hopeful", "hungry", "infallible", "inspiring", "intelligent", "interesting", "jolly", 
		"jovial", "keen", "kind", "laughing", "loving", "lucid", "magical", "modest", "musing", "mystifying", "naughty", "nervous", "nice", 
		"nifty", "nostalgic", "objective", "optimistic", "peaceful", "pedantic", "pensive", "practical", "priceless", "quirky", "quizzical", 
		"recursing", "relaxed", "reverent", "romantic", "sad", "serene", "sharp", "silly", "sleepy", "stoic", "strange", "stupefied", "suspicious", 
		"sweet", "tender", "thirsty", "trusting", "unruffled", "upbeat", "vibrant", "vigilant", "vigorous", "wizardly", "wonderful", "xenodochial", 
		"youthful", "zealous", "zen"];
    #[rustfmt::skip]
    static NAMES: [&str; 241] = ["agnesi", "albattani", "allen", "almeida", "antonelli", "archimedes", "ardinghelli", "aryabhata", "austin", 
		"babbage", "banach", "banzai", "bardeen", "bartik", "bassi", "beaver", "bell", "benz", "bhabha", "bhaskara", "black", "blackburn", "blackwell", 
		"bohr", "booth", "borg", "bose", "bouman", "boyd", "brahmagupta", "brattain", "brown", "buck", "burnell", "cannon", "carson", "cartwright", 
		"carver", "cauchy", "cerf", "chandrasekhar", "chaplygin", "chatelet", "chatterjee", "chaum", "chebyshev", "clarke", "cohen", "colden", "cori", 
		"cray", "curie", "curran", "darwin", "davinci", "dewdney", "dhawan", "diffie", "dijkstra", "dirac", "driscoll", "dubinsky", "easley", "edison", 
		"einstein", "elbakyan", "elgamal", "elion", "ellis", "engelbart", "euclid", "euler", "faraday", "feistel", "fermat", "fermi", "feynman", "franklin", 
		"gagarin", "galileo", "galois", "ganguly", "gates", "gauss", "germain", "goldberg", "goldstine", "goldwasser", "golick", "goodall", "gould", "greider", 
		"grothendieck", "haibt", "hamilton", "hardy", "haslett", "hawking", "heisenberg", "hellman", "hermann", "herschel", "hertz", "heyrovsky", "hodgkin", 
		"hofstadter", "hoover", "hopper", "hugle", "hypatia", "ishizaka", "jackson", "jang", "jemison", "jennings", "jepsen", "johnson", "joliot", "jones", 
		"kalam", "kapitsa", "kare", "keldysh", "keller", "kepler", "khayyam", "khorana", "kilby", "kirch", "knuth", "kowalevski", "lalande", "lamarr", 
		"lamport", "leakey", "leavitt", "lederberg", "lehmann", "lewin", "lichterman", "liskov", "lovelace", "lumiere", "mahavira", "margulis", "matsumoto", 
		"maxwell", "mayer", "mccarthy", "mcclintock", "mclaren", "mclean", "mcnulty", "meitner", "mendel", "mendeleev", "meninsky", "merkle", "mestorf", 
		"mirzakhani", "montalcini", "moore", "morse", "moser", "murdock", "napier", "nash", "neumann", "newton", "nightingale", "nobel", "noether", "northcutt", 
		"noyce", "panini", "pare", "pascal", "pasteur", "payne", "perlman", "pike", "poincare", "poitras", "proskuriakova", "ptolemy", "raman", "ramanujan", 
		"rhodes", "ride", "riemann", "ritchie", "robinson", "roentgen", "rosalind", "rubin", "saha", "sammet", "sanderson", "satoshi", "shamir", "shannon", 
		"shaw", "shirley", "shockley", "shtern", "sinoussi", "snyder", "solomon", "spence", "stonebraker", "sutherland", "swanson", "swartz", "swirles", 
		"taussig", "tesla", "tharp", "thompson", "torvalds", "tu", "turing", "varahamihira", "vaughan", "vaughn", "villani", "visvesvaraya", "volhard", 
		"wescoff", "weierstrass", "wilbur", "wiles", "williams", "williamson", "wilson", "wing", "wozniak", "wright", "wu", "yalow", "yonath", "zhukovsky"];

    static CHARS_SMALL: [char; 36] = [
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ];

    // uppercase and lowercase characters; exclude ambiguous characters
    static CHARS_CAPITAL: [char; 58] = [
        'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'S', 'T',
        'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'm',
        'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '1', '2', '3', '4', '5',
        '6', '7', '8', '9',
    ];

    if style == "UID" {
        if allow_capital_letters {
            nanoid!(len, &CHARS_CAPITAL)
        } else {
            nanoid!(len, &CHARS_SMALL)
        }
    } else {
        format!(
            "{0}-{1}",
            ADJECTIVES
                .choose(&mut rand::rng())
                .expect("Error choosing random adjective."),
            NAMES
                .choose(&mut rand::rng())
                .expect("Error choosing random name.")
        )
    }
}

## ADDED Requirements
### Requirement: Ads catalog
The system SHALL provide an ads catalog stored in the database with reusable ads records.

#### Scenario: Ads CRUD via admin
- **WHEN** an admin uses the Ads tab to manage ads
- **THEN** the system SHALL allow creating, editing, listing, and deleting ads.

#### Scenario: Required ad fields
- **WHEN** an ad is created or edited
- **THEN** the system SHALL require id, a unique name, an image URL, an ad destination link URL (not the short link), an expiry value, and a `countdown_seconds` value within 0–30 seconds inclusive.

#### Scenario: Default countdown applied
- **WHEN** an ad is created without specifying `countdown_seconds`
- **THEN** the system SHALL set `countdown_seconds` to 5 seconds.

#### Scenario: Countdown validated
- **WHEN** an ad is created or edited with countdown seconds outside allowed bounds
- **THEN** the system SHALL reject the request with validation errors when outside 0–30 seconds (inclusive).

#### Scenario: Duplicate ad name rejected
- **WHEN** an admin attempts to create or rename an ad to a name already in use
- **THEN** the system SHALL reject the request and preserve existing records.

#### Scenario: Ad link format permissive
- **WHEN** an ad is created or edited
- **THEN** the system SHALL accept the ad link field without enforcing URL format restrictions.

#### Scenario: Expired ads filtered
- **WHEN** an ad is expired
- **THEN** the system SHALL treat it as inactive and omit it from lists intended for selection/use while still displaying it in admin CRUD lists.


#### Scenario: Default countdown applied
- **WHEN** a ad is created without specifying countdown seconds
- **THEN** the system SHALL set `countdown_seconds` to 5 seconds.

## ADDED Requirements
### Requirement: Optional ad association on links
Short links SHALL support an optional association to one catalog ad.

#### Scenario: Link created without ad
- **WHEN** an admin creates a link without selecting an ad
- **THEN** the link SHALL save successfully with no ad association.

#### Scenario: Link created with ad
- **WHEN** an admin creates or edits a link and selects a valid ad
- **THEN** the link SHALL persist the association and expose it via list/detail APIs.

#### Scenario: Invalid ad rejected
- **WHEN** an admin attempts to create or edit a link with an ad that is missing or inactive
- **THEN** the request SHALL be rejected with validation errors.

#### Scenario: Ads delete
- **WHEN** an ad is delete
- **THEN** reset DBRow.ad_id whice got this id to None
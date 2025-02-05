# Data Transformations for Supabase FDW

This document outlines the necessary data transformations when migrating the SAM.gov opportunities data to a Supabase Foreign Data Wrapper (FDW) using Rust.

## Header Mapping

```rust
const COLUMN_MAPPING: &[(&str, &str)] = &[
    ("NoticeId", "notice_id"),
    ("Title", "title"),
    ("Sol#", "solicitation_number"),
    ("Department/Ind.Agency", "agency"),
    ("CGAC", "cgac"),
    ("Sub-Tier", "sub_tier"),
    ("FPDS Code", "fpds_code"),
    ("Office", "office"),
    ("AAC Code", "aac_code"),
    ("PostedDate", "posted_date"),
    ("Type", "type"),
    ("BaseType", "base_type"),
    ("ArchiveType", "archive_type"),
    ("ArchiveDate", "archive_date"),
    ("SetASideCode", "set_aside_code"),
    ("SetASide", "set_aside"),
    ("ResponseDeadLine", "response_deadline"),
    ("NaicsCode", "naics_code"),
    ("ClassificationCode", "classification_code"),
    ("PopStreetAddress", "pop_street_address"),
    ("PopCity", "pop_city"),
    ("PopState", "pop_state"),
    ("PopZip", "pop_zip"),
    ("PopCountry", "pop_country"),
    ("Active", "active"),
    ("AwardNumber", "award_number"),
    ("AwardDate", "award_date"),
    ("Award$", "award_amount"),
    ("Awardee", "awardee"),
    ("PrimaryContactTitle", "primary_contact_title"),
    ("PrimaryContactFullname", "primary_contact_fullname"),
    ("PrimaryContactEmail", "primary_contact_email"),
    ("PrimaryContactPhone", "primary_contact_phone"),
    ("PrimaryContactFax", "primary_contact_fax"),
    ("SecondaryContactTitle", "secondary_contact_title"),
    ("SecondaryContactFullname", "secondary_contact_fullname"),
    ("SecondaryContactEmail", "secondary_contact_email"),
    ("SecondaryContactPhone", "secondary_contact_phone"),
    ("SecondaryContactFax", "secondary_contact_fax"),
    ("OrganizationType", "organization_type"),
    ("State", "state"),
    ("City", "city"),
    ("ZipCode", "zip_code"),
    ("CountryCode", "country_code"),
    ("AdditionalInfoLink", "additional_info_link"),
    ("Link", "link"),
    ("Description", "description"),
];
```

## Column Types

```rust
const COLUMN_TYPES: &[(&str, &str)] = &[
    ("notice_id", "TEXT PRIMARY KEY"),
    ("title", "TEXT"),
    ("description", "TEXT"),
    ("agency", "TEXT"),
    ("solicitation_number", "TEXT"),
    ("response_deadline", "TIMESTAMP WITH TIME ZONE"),
    ("posted_date", "TIMESTAMP WITH TIME ZONE"),
    ("cgac", "TEXT"),
    ("sub_tier", "TEXT"),
    ("fpds_code", "TEXT"),
    ("office", "TEXT"),
    ("aac_code", "TEXT"),
    ("type", "TEXT"),
    ("base_type", "TEXT"),
    ("archive_type", "TEXT"),
    ("archive_date", "DATE"),
    ("set_aside_code", "TEXT"),
    ("set_aside", "TEXT"),
    ("naics_code", "TEXT"),
    ("classification_code", "TEXT"),
    ("pop_street_address", "TEXT"),
    ("pop_city", "TEXT"),
    ("pop_state", "TEXT"),
    ("pop_zip", "TEXT"),
    ("pop_country", "TEXT"),
    ("active", "BOOLEAN"),
    ("award_number", "TEXT"),
    ("award_date", "DATE"),
    ("award_amount", "NUMERIC"),
    ("awardee", "TEXT"),
    ("primary_contact_title", "TEXT"),
    ("primary_contact_fullname", "TEXT"),
    ("primary_contact_email", "TEXT"),
    ("primary_contact_phone", "TEXT"),
    ("primary_contact_fax", "TEXT"),
    ("secondary_contact_title", "TEXT"),
    ("secondary_contact_fullname", "TEXT"),
    ("secondary_contact_email", "TEXT"),
    ("secondary_contact_phone", "TEXT"),
    ("secondary_contact_fax", "TEXT"),
    ("organization_type", "TEXT"),
    ("state", "TEXT"),
    ("city", "TEXT"),
    ("zip_code", "TEXT"),
    ("country_code", "TEXT"),
    ("additional_info_link", "TEXT"),
    ("link", "TEXT")
];
```

## Date and Timestamp Transformations

```rust
// posted_date, response_deadline -> TIMESTAMP WITH TIME ZONE
- Convert to UTC timezone
- Format as "YYYY-MM-DD HH:MM:SS"
- Null if invalid/empty

// archive_date, award_date -> DATE
- Format as "YYYY-MM-DD" 
- Null if invalid/empty
```

## Numeric Field Transformations

```rust
// award_amount -> NUMERIC
- Strip "$" and "," 
- Convert to float
- Null if invalid/empty

// naics_code, cgac -> TEXT (not NUMERIC)
- Convert float to integer then string to remove ".0"
- Example: "123.0" -> "123"
- Null if invalid/empty
```

## Boolean Field Transformations

```rust
// active -> BOOLEAN
"Yes" -> true
"No" -> false
anything else -> null
```

## Text Field Transformations

```rust
// All text fields
- Empty string -> null
- "nan" (case insensitive) -> null
- pd.NA -> null
```

## Primary Key Handling

```rust
// notice_id -> TEXT PRIMARY KEY
- Must be non-null and non-empty
- Records with null/empty notice_id are filtered out
```

## Encoding Transformations

The source data uses Latin-1 encoding which needs to be handled properly in the Rust implementation:

```rust
// Reading CSV with proper encoding
let reader = csv::ReaderBuilder::new()
    .encoding(Some(encoding_rs::WINDOWS_1252))  // Latin-1/Windows-1252
    .from_path(path)?;

// Convert to UTF-8
let text = String::from_utf8_lossy(bytes);

// Optional: Validate UTF-8
fn is_valid_utf8(s: &str) -> bool {
    String::from_utf8(s.as_bytes().to_vec()).is_ok()
}
```

### Encoding Considerations

- Handle or replace non-UTF8 characters
- Use replacement character (�) for invalid UTF-8 sequences
- Implement custom error handling for encoding failures
- Pay special attention to `description` and `title` fields which may contain special characters

## Key Validation Rules

1. Skip records with invalid/missing primary key
2. Convert all empty strings to null
3. Handle "nan" values (case insensitive) as null
4. Ensure timestamps are in UTC
5. Strip currency formatting from numeric fields

## Potential Gotchas

### Memory Management

- Large description fields can cause memory spikes
- Consider streaming/chunking large text fields
- Watch for memory leaks in UTF-8 conversions

### Data Integrity

- Duplicate notice_ids might exist in source data
- Some fields might contain HTML or markdown formatting
- Phone/fax numbers have inconsistent formatting
- ZIP codes might be partial (5 digit vs 9 digit)
- State codes might be full names or abbreviations

### Timezone Handling

- Some dates might be in local timezone instead of UTC
- DST transitions can cause duplicate or missing hours
- Some dates might be invalid (e.g., Feb 30)

### CSV Parsing

- Some fields contain commas within unquoted strings
- Line breaks within fields can break row parsing
- Hidden/control characters in text fields
- Inconsistent quote escaping

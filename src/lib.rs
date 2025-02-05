#[allow(warnings)]
mod bindings;

use chrono::prelude::*;
use csv::ReaderBuilder;
use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;
use std::io::Cursor;

use bindings::{
    exports::supabase::wrappers::routines::Guest,
    supabase::wrappers::{
        http,
        types::{Cell, Context, FdwError, FdwResult, OptionsType, Row},
    },
};

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

#[derive(Default)]
struct SamFDW {
    base_url: String,
    csv_reader: Option<csv::Reader<Box<dyn std::io::Read + Send>>>,
    headers: Vec<String>,
    response_body: Vec<u8>,
}

impl std::fmt::Debug for SamFDW {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamFDW")
            .field("base_url", &self.base_url)
            .field("headers", &self.headers)
            .field("response_body", &self.response_body)
            .finish()
    }
}

// pointer for the static FDW instance
static mut INSTANCE: *mut SamFDW = std::ptr::null_mut::<SamFDW>();

impl SamFDW {
    // initialise FDW instance
    fn init_instance() {
        let instance = Self::default();
        unsafe {
            INSTANCE = Box::leak(Box::new(instance));
        }
    }

    fn this_mut() -> &'static mut Self {
        unsafe { &mut (*INSTANCE) }
    }

    fn transform_value(col_name: &str, value: &str) -> Option<Cell> {
        if value.trim().is_empty() || value.to_lowercase() == "nan" {
            return None;
        }

        match col_name {
            "posted_date" | "response_deadline" => {
                if let Ok(dt) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
                    let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
                    Some(Cell::Timestamp(utc_dt.timestamp()))
                } else {
                    None
                }
            }

            "archive_date" | "award_date" => {
                if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
                    let dt = date.and_hms_opt(0, 0, 0).unwrap();
                    let ts = DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
                    Some(Cell::Timestamp(ts.timestamp()))
                } else {
                    None
                }
            }

            "award_amount" => {
                let cleaned = value.replace(['$', ','], "");
                cleaned.parse::<f64>().ok().map(|v| Cell::Numeric(v))
            }

            "naics_code" | "cgac" => {
                if let Ok(num) = value.parse::<f64>() {
                    Some(Cell::String(num.trunc().to_string()))
                } else {
                    Some(Cell::String(value.to_string()))
                }
            }

            "active" => match value.to_lowercase().as_str() {
                "yes" => Some(Cell::Bool(true)),
                "no" => Some(Cell::Bool(false)),
                _ => None,
            },

            _ => Some(Cell::String(value.to_string())),
        }
    }

    fn get_column_index(&self, target_col: &str) -> Option<usize> {
        COLUMN_MAPPING
            .iter()
            .find(|(_, tgt)| *tgt == target_col)
            .and_then(|(src, _)| self.headers.iter().position(|h| h.as_str() == *src))
    }
}

impl Guest for SamFDW {
    fn host_version_requirement() -> String {
        // semver expression for Wasm FDW host version requirement
        // ref: https://docs.rs/semver/latest/semver/enum.Op.html
        "^0.1.0".to_string()
    }

    fn init(ctx: &Context) -> FdwResult {
        Self::init_instance();
        let this = Self::this_mut();

        let opts = ctx.get_options(OptionsType::Server);
        this.base_url = opts.require_or("api_url", "https://falextracts.s3.amazonaws.com");

        Ok(())
    }

    fn begin_scan(ctx: &Context) -> FdwResult {
        let this = Self::this_mut();

        // Only fetch data if we don't have it already
        if this.response_body.is_empty() {
            let url = format!(
                "{}/Contract%20Opportunities/datagov/ContractOpportunitiesFullCSV.csv",
                this.base_url
            );
            let headers = vec![("user-agent".to_owned(), "SAM FDW".to_owned())];
            let req = http::Request {
                method: http::Method::Get,
                url,
                headers,
                body: String::default(),
            };
            let resp = http::get(&req)?;
            this.response_body = resp.body.into_bytes();
        }

        // Create a new reader from the stored response
        let cursor = Cursor::new(&this.response_body);
        let transcoded = DecodeReaderBytesBuilder::new()
            .encoding(Some(WINDOWS_1252))
            .build(cursor);

        let mut rdr = ReaderBuilder::new()
            .flexible(true)
            .from_reader(Box::new(transcoded) as Box<dyn std::io::Read + Send>);

        // Get headers if we don't have them
        if this.headers.is_empty() {
            this.headers = rdr
                .headers()
                .map_err(|e| e.to_string())?
                .iter()
                .map(|s| s.to_string())
                .collect();
        }

        this.csv_reader = Some(rdr);
        Ok(())
    }

    fn iter_scan(ctx: &Context, row: &Row) -> Result<Option<u32>, FdwError> {
        let this = Self::this_mut();

        let reader = this
            .csv_reader
            .as_mut()
            .ok_or("CSV reader not initialized")?;

        let mut record = csv::StringRecord::new();
        match reader.read_record(&mut record) {
            Ok(true) => {
                // Skip rows without notice_id
                if let Some(notice_id_idx) = this.get_column_index("notice_id") {
                    if record
                        .get(notice_id_idx)
                        .map_or(true, |v| v.trim().is_empty())
                    {
                        return Ok(Some(0));
                    }
                }

                for tgt_col in ctx.get_columns() {
                    let tgt_col_name = tgt_col.name();

                    if let Some(idx) = this.get_column_index(tgt_col_name.as_str()) {
                        if let Some(value) = record.get(idx) {
                            let cell = Self::transform_value(tgt_col_name.as_str(), value);
                            row.push(cell.as_ref());
                        } else {
                            row.push(None);
                        }
                    } else {
                        return Err(format!("Column mapping not found for '{}'", tgt_col_name));
                    }
                }
                Ok(Some(0))
            }
            Ok(false) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn re_scan(ctx: &Context) -> FdwResult {
        // Re-initialize the CSV reader from stored response
        Self::begin_scan(ctx)
    }

    fn end_scan(_ctx: &Context) -> FdwResult {
        let this = Self::this_mut();
        this.csv_reader = None;
        // Don't clear headers or response_body as we might need them for re-scanning
        Ok(())
    }

    fn begin_modify(_ctx: &Context) -> FdwResult {
        Err("modify on foreign table is not supported".to_owned())
    }

    fn insert(_ctx: &Context, _row: &Row) -> FdwResult {
        Ok(())
    }

    fn update(_ctx: &Context, _rowid: Cell, _row: &Row) -> FdwResult {
        Ok(())
    }

    fn delete(_ctx: &Context, _rowid: Cell) -> FdwResult {
        Ok(())
    }

    fn end_modify(_ctx: &Context) -> FdwResult {
        Ok(())
    }
}

bindings::export!(SamFDW
 with_types_in bindings);

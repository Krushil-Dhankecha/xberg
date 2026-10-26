//! OfficeArt (MS-ODRAW) records, shared by the legacy binary Office formats.
//!
//! DOC and PPT store their drawings and pictures as OfficeArt record trees. A PowerPoint
//! record (MS-PPT 2.3.1) has the same 8-byte header, so the walkers here also serve the
//! `PowerPoint Document` stream. ~keep

use crate::extractors::security::{SecurityBudget, SecurityError};

/// `OfficeArtBlip` record types for the raster formats (MS-ODRAW 2.2.23).
/// `RT_BLIP_JPEG_ALT` (0xF02A) is an alternate `recType` documented for JPEG blips
/// written by older Office versions; it uses the same `OfficeArtBlipJPEG` layout as
/// 0xF01D.
pub(crate) const RT_BLIP_JPEG: u16 = 0xF01D;
const RT_BLIP_JPEG_ALT: u16 = 0xF02A;
pub(crate) const RT_BLIP_PNG: u16 = 0xF01E;
pub(crate) const RT_BLIP_DIB: u16 = 0xF01F;

/// `OfficeArtSpgrContainer`: a group of shapes. PowerPoint 97-2003 draws a table as one.
pub(crate) const MSOFBT_SPGR_CONTAINER: u16 = 0xF003;
/// `OfficeArtSpContainer`: one shape.
pub(crate) const MSOFBT_SP_CONTAINER: u16 = 0xF004;
/// `OfficeArtFSP`; its `recInstance` is the shape type.
pub(crate) const MSOFBT_FSP: u16 = 0xF00A;

/// `msofbtBSE` -- one per blip in the document's `BStoreContainer`. A shape's `pib`
/// property is a 1-based index into these, in container order (MS-ODRAW 2.2.32). ~keep
pub(crate) const MSOFBT_BSE: u16 = 0xF007;
/// Byte offset of `foDelay` inside an `msofbtBSE`'s content, past `btWin32` (1), `btMacOS`
/// (1), `rgbUid` (16), `tag` (2), `size` (4) and `cRef` (4). `foDelay` is the blip record's
/// own start offset in the delay stream: `Pictures` in a PPT, `WordDocument` in a DOC. ~keep
const BSE_FO_DELAY_OFFSET: usize = 28;
/// Byte offset of `cbName` inside an `msofbtBSE`'s content: the length of the name that
/// follows the fixed part.
const BSE_NAME_LENGTH_OFFSET: usize = 33;
/// Length of the fixed part of an `msofbtBSE`'s content; the name, then the blip when the
/// file block holds it, follow.
const BSE_HEADER_LEN: usize = 36;

/// `msofbtOPT` and its secondary/tertiary siblings, each a table of `OfficeArtFOPTE`
/// property entries (MS-ODRAW 2.3.1). `pib` is normally in the primary table, but a
/// shape that overflows its property set spills into the other two. ~keep
pub(crate) const MSOFBT_OPT: [u16; 3] = [0xF00B, 0xF121, 0xF122];
/// An `OfficeArtFOPTE` is a 2-byte `opid` followed by a 4-byte value.
const FOPT_ENTRY_LEN: usize = 6;
/// The low 14 bits of `opid` are the property number; bit 14 is `fBid` and bit 15 is
/// `fComplex`, neither of which changes where the entry sits. ~keep
const FOPT_PROPERTY_ID_MASK: u16 = 0x3FFF;
/// `pib` -- the 1-based `BStoreContainer` index of the blip a picture shape displays.
pub(crate) const MSO_PROPERTY_PIB: u16 = 0x0104;

/// Depth cap for the OfficeArt container walk. Real nesting is under a dozen levels; this
/// only stops a hostile self-describing container tree from exhausting the stack. ~keep
const MAX_OFFICE_ART_DEPTH: usize = 16;

/// One record header from the PowerPoint/OfficeArt record tree, flattened out of its
/// containers so callers can filter by type without re-walking.
#[derive(Clone, Copy)]
pub(crate) struct ArtRecord {
    rec_ver: u16,
    pub(crate) rec_instance: u16,
    pub(crate) rec_type: u16,
    pub(crate) content_start: usize,
    pub(crate) content_end: usize,
}

impl ArtRecord {
    /// A record is a container when `recVer` (the low nibble of the first field) is 0xF;
    /// its children are the records inside its own declared byte range.
    fn is_container(&self) -> bool {
        self.rec_ver == 0x0F
    }

    /// The record's content within the `data` it was read from.
    fn content<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        data.get(self.content_start..self.content_end).unwrap_or_default()
    }
}

/// The record whose header starts at `data[pos]`, or `None` when its header or content
/// would run past `end`.
fn record_at(data: &[u8], pos: usize, end: usize) -> Option<ArtRecord> {
    let content_start = pos.checked_add(8).filter(|&start| start <= end)?;
    let header = data.get(pos..content_start)?;
    let rec_ver_instance = u16::from_le_bytes([header[0], header[1]]);
    let rec_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let content_end = content_start
        .checked_add(rec_len)
        .filter(|&candidate| candidate <= end)?;
    Some(ArtRecord {
        rec_ver: rec_ver_instance & 0x000F,
        rec_instance: rec_ver_instance >> 4,
        rec_type: u16::from_le_bytes([header[2], header[3]]),
        content_start,
        content_end,
    })
}

/// Collect every record header in `data[start..end]`, descending into containers.
///
/// A `recLen` that would run past `end` stops the walk at that level rather than
/// over-reading -- the stream is untrusted, and a truncated tail is the common shape of a
/// salvageable corrupt file.
pub(crate) fn collect_art_records(data: &[u8], start: usize, end: usize, depth: usize, out: &mut Vec<ArtRecord>) {
    if depth > MAX_OFFICE_ART_DEPTH {
        return;
    }
    let mut pos = start;
    while let Some(record) = record_at(data, pos, end) {
        out.push(record);
        if record.is_container() {
            collect_art_records(data, record.content_start, record.content_end, depth + 1, out);
        }
        pos = record.content_end;
    }
}

/// The records directly inside `data[start..end]`, without descending into containers.
pub(crate) fn child_records(data: &[u8], start: usize, end: usize) -> Vec<ArtRecord> {
    let mut out = Vec::new();
    let mut pos = start;
    while let Some(record) = record_at(data, pos, end) {
        out.push(record);
        pos = record.content_end;
    }
    out
}

/// The `(opid, value)` entries of a property table, whose `recInstance` is the entry count.
/// Empty when `record` is not a property table; entries that do not fit in the record are
/// not returned.
fn fopt_entries<'a>(data: &'a [u8], record: &ArtRecord) -> impl Iterator<Item = (u16, u32)> + 'a {
    let count = if MSOFBT_OPT.contains(&record.rec_type) {
        usize::from(record.rec_instance)
    } else {
        0
    };
    record
        .content(data)
        .as_chunks::<FOPT_ENTRY_LEN>()
        .0
        .iter()
        .take(count)
        .map(|entry| {
            (
                u16::from_le_bytes([entry[0], entry[1]]),
                u32::from_le_bytes([entry[2], entry[3], entry[4], entry[5]]),
            )
        })
}

/// The value of `property` in a shape's own property tables, if they set it.
pub(crate) fn shape_property(data: &[u8], shape: &ArtRecord, property: u16) -> Option<u32> {
    child_records(data, shape.content_start, shape.content_end)
        .iter()
        .find_map(|record| {
            fopt_entries(data, record)
                .find(|(opid, _)| opid & FOPT_PROPERTY_ID_MASK == property)
                .map(|(_, value)| value)
        })
}

/// The non-zero `pib` values a property table sets.
pub(crate) fn pib_values<'a>(data: &'a [u8], record: &ArtRecord) -> impl Iterator<Item = u32> + 'a {
    fopt_entries(data, record)
        .filter(|(opid, _)| opid & FOPT_PROPERTY_ID_MASK == MSO_PROPERTY_PIB)
        .map(|(_, pib)| pib)
        .filter(|&pib| pib > 0)
}

/// `foDelay` of an `msofbtBSE`, or `None` when `record` is not one or is too short to hold
/// it.
fn bse_fo_delay(data: &[u8], record: &ArtRecord) -> Option<u32> {
    if record.rec_type != MSOFBT_BSE {
        return None;
    }
    let bytes = record.content(data).get(BSE_FO_DELAY_OFFSET..BSE_FO_DELAY_OFFSET + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// The blips of a document, in order, so that a shape's 1-based `pib` indexes straight
/// into them.
pub(crate) struct BlipStore<'a> {
    data: &'a [u8],
    blocks: Vec<ArtRecord>,
}

impl<'a> BlipStore<'a> {
    /// Every `msofbtBSE` among `records` that is long enough to hold `foDelay`, in stream
    /// order. This is how the PPT picture map has always numbered blips: a BSE too short to
    /// hold `foDelay` is skipped and does not take a `pib`.
    pub(crate) fn from_bse_records(data: &'a [u8], records: &[ArtRecord]) -> Self {
        let blocks = records
            .iter()
            .filter(|record| bse_fo_delay(data, record).is_some())
            .copied()
            .collect();
        Self { data, blocks }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// `foDelay` of the blip `pib` names.
    pub(crate) fn fo_delay(&self, pib: u32) -> Option<u32> {
        let index = usize::try_from(pib).ok()?.checked_sub(1)?;
        bse_fo_delay(self.data, self.blocks.get(index)?)
    }
}

/// A raster picture format a blip can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlipFormat {
    Jpeg,
    Png,
    Dib,
}

impl BlipFormat {
    /// The format name an `ExtractedImage` carries.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Dib => "dib",
        }
    }
}

/// The raster format of a blip record type, or `None` for a metafile blip or any other
/// record.
pub(crate) fn blip_format(rec_type: u16) -> Option<BlipFormat> {
    match rec_type {
        RT_BLIP_JPEG | RT_BLIP_JPEG_ALT => Some(BlipFormat::Jpeg),
        RT_BLIP_PNG => Some(BlipFormat::Png),
        RT_BLIP_DIB => Some(BlipFormat::Dib),
        _ => None,
    }
}

/// Length of a raster blip's header before its picture bytes.
///
/// One `rgbUid` (16 bytes) + `tag` (1 byte) = 17-byte header, or two `rgbUid`s + `tag` =
/// 33 bytes; per MS-ODRAW 2.2.27-2.2.29 the low bit of `recInstance` is what distinguishes
/// the two UID counts for every raster blip type (e.g. JPEG 0x46A vs 0x46B, PNG 0x6E0 vs
/// 0x6E1, DIB 0x7A8 vs 0x7A9).
pub(crate) fn blip_header_len(rec_instance: u16) -> usize {
    if rec_instance & 0x1 == 1 { 33 } else { 17 }
}

/// The first blip in `data` whose format is one of `formats`, with its picture bytes.
///
/// `data` is a run of OfficeArt records; containers and the file blocks of a
/// `BStoreContainer` are searched in turn. A blip in another format is skipped, and a
/// malformed record ends the search of the records around it.
pub(crate) fn find_blip<'a>(
    data: &'a [u8],
    formats: &[BlipFormat],
    budget: &mut SecurityBudget,
) -> Result<Option<(BlipFormat, &'a [u8])>, SecurityError> {
    let mut position = 0;
    while position < data.len() {
        budget.step()?;
        let Some(record) = record_at(data, position, data.len()) else {
            return Ok(None);
        };
        let content = record.content(data);
        if record.is_container() || record.rec_type == MSOFBT_BSE {
            let children = if record.rec_type == MSOFBT_BSE {
                let Some(name_length) = content.get(BSE_NAME_LENGTH_OFFSET) else {
                    return Ok(None);
                };
                content.get(BSE_HEADER_LEN + usize::from(*name_length)..)
            } else {
                Some(content)
            };
            budget.enter()?;
            let result = match children {
                Some(children) => find_blip(children, formats, budget)?,
                None => None,
            };
            budget.leave();
            if result.is_some() {
                return Ok(result);
            }
        } else if let Some(format) = blip_format(record.rec_type).filter(|format| formats.contains(format)) {
            let Some(bytes) = content
                .get(blip_header_len(record.rec_instance)..)
                .filter(|bytes| !bytes.is_empty())
            else {
                return Ok(None);
            };
            return Ok(Some((format, bytes)));
        }
        position = record.content_end;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::security::SecurityLimits;

    fn record(rec_ver_instance: u16, rec_type: u16, content: &[u8]) -> Vec<u8> {
        let mut bytes = rec_ver_instance.to_le_bytes().to_vec();
        bytes.extend_from_slice(&rec_type.to_le_bytes());
        bytes.extend_from_slice(&(content.len() as u32).to_le_bytes());
        bytes.extend_from_slice(content);
        bytes
    }

    #[test]
    fn should_descend_into_containers_and_stop_at_a_record_that_overruns() {
        let mut data = record(0x000F, MSOFBT_SP_CONTAINER, &record(0x0002, MSOFBT_FSP, &[0; 8]));
        data.extend(record(0x0000, MSOFBT_BSE, &[0; 4]));
        let truncated = data.len();
        data.extend(record(0x0000, MSOFBT_BSE, &[0; 4]));
        data.truncate(data.len() - 1);

        let mut records = Vec::new();
        collect_art_records(&data, 0, data.len(), 0, &mut records);
        let types: Vec<u16> = records.iter().map(|record| record.rec_type).collect();
        assert_eq!(types, [MSOFBT_SP_CONTAINER, MSOFBT_FSP, MSOFBT_BSE]);
        assert!(records[0].is_container());
        assert_eq!(records[1].content(&data), [0; 8]);
        assert_eq!(child_records(&data, 0, data.len()).len(), 2);
        assert!(record_at(&data, truncated, data.len()).is_none());
        assert!(record_at(&data, usize::MAX, data.len()).is_none());
    }

    #[test]
    fn should_read_shape_property_from_every_property_table_within_its_entry_count() {
        let entry = |opid: u16, value: u32| [opid.to_le_bytes().as_slice(), &value.to_le_bytes()].concat();
        // Two entries declared, three present: the third is complex data, not an entry. ~keep
        let primary = [entry(0x0080, 7), entry(0x0081, 8), entry(MSO_PROPERTY_PIB, 9)].concat();
        let tertiary = entry(MSO_PROPERTY_PIB | 0x4000, 3);
        let mut shape = record(0x0023, MSOFBT_OPT[0], &primary);
        shape.extend(record(0x0013, MSOFBT_OPT[2], &tertiary));
        let data = record(0x000F, MSOFBT_SP_CONTAINER, &shape);
        let container = record_at(&data, 0, data.len()).expect("shape container");

        assert_eq!(shape_property(&data, &container, 0x0081), Some(8));
        assert_eq!(shape_property(&data, &container, MSO_PROPERTY_PIB), Some(3));
        assert_eq!(shape_property(&data, &container, 0x0182), None);
    }

    #[test]
    fn should_number_only_the_bse_records_that_hold_fo_delay() {
        let bse = |fo_delay: u32| {
            let mut content = vec![0; BSE_FO_DELAY_OFFSET];
            content.extend_from_slice(&fo_delay.to_le_bytes());
            record(0x0002, MSOFBT_BSE, &content)
        };
        let mut data = bse(10);
        data.extend(record(0x0002, MSOFBT_BSE, &[0; BSE_FO_DELAY_OFFSET + 3]));
        data.extend(record(0x0000, MSOFBT_FSP, &[0; 8]));
        data.extend(bse(20));
        let mut records = Vec::new();
        collect_art_records(&data, 0, data.len(), 0, &mut records);
        let store = BlipStore::from_bse_records(&data, &records);

        assert_eq!(store.fo_delay(1), Some(10));
        assert_eq!(store.fo_delay(2), Some(20));
        assert_eq!(store.fo_delay(3), None);
        assert_eq!(store.fo_delay(0), None);
    }

    #[test]
    fn should_find_blip_in_file_blocks_and_skip_formats_not_asked_for() {
        let blip = |rec_instance: u16, rec_type: u16, picture: &[u8]| {
            let header = vec![0; blip_header_len(rec_instance)];
            record(rec_instance << 4, rec_type, &[header.as_slice(), picture].concat())
        };
        let mut bse = vec![0; BSE_HEADER_LEN];
        bse[BSE_NAME_LENGTH_OFFSET] = 2;
        bse.extend_from_slice(b"nm");
        bse.extend(blip(0x6E1, RT_BLIP_PNG, b"png"));
        let mut data = blip(0x7A8, RT_BLIP_DIB, b"dib");
        data.extend(record(0x000F, 0xF001, &record(0x0002, MSOFBT_BSE, &bse)));
        let limits = SecurityLimits::default();

        let found = find_blip(&data, &[BlipFormat::Png], &mut SecurityBudget::from_limits(&limits));
        assert_eq!(found.expect("budget"), Some((BlipFormat::Png, b"png".as_slice())));
        let found = find_blip(&data, &[BlipFormat::Dib], &mut SecurityBudget::from_limits(&limits));
        assert_eq!(found.expect("budget"), Some((BlipFormat::Dib, b"dib".as_slice())));
        let found = find_blip(
            &data[..data.len() - 1],
            &[BlipFormat::Png],
            &mut SecurityBudget::from_limits(&limits),
        );
        assert_eq!(found.expect("budget"), None);
    }

    #[test]
    fn should_size_blip_header_for_one_or_two_uids() {
        assert_eq!(blip_header_len(0x6E0), 17);
        assert_eq!(blip_header_len(0x6E1), 33);
    }
}

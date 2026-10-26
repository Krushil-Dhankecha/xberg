//! Live inline-picture references in Word's CHPX/PICF records, and floating
//! pictures anchored through `PlcSpaMom` and the OfficeArt drawing group. ~keep

use super::{DOC_WARNING_SOURCE, DocExtractionResult, DocParagraph};
use crate::error::{Result, XbergError};
use crate::extraction::office_art::{
    ArtRecord, Blip, BlipFormat, BlipStore, MSOFBT_DG_CONTAINER, MSOFBT_DGG_CONTAINER, Shape, drawing_shapes,
    find_blip, record_at,
};
use crate::extractors::security::{SecurityBudget, SecurityLimits};
use crate::types::{ExtractedImage, ProcessingWarning};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

// [MS-DOC] CHPX/PICF record identifiers. ~keep
const CHPX_FIB_PAIR: usize = 12;
const FKP_PAGE_BYTES: usize = 512;
const FKP_PAGE_MASK: u32 = 0x003f_ffff;
const PICF_HEADER_BYTES: usize = 68;
const MM_SHAPE: u16 = 100;
const MM_SHAPEFILE: u16 = 102;
const SPRM_C_SPEC: u16 = 0x0855;
const SPRM_C_DATA: u16 = 0x0806;
const SPRM_C_PIC_LOCATION: u16 = 0x6a03;
// [MS-DOC] FibRgFcLcb97 pairs, counted from `fcStshfOrig` (0) as `fcClx` (33) is in
// mod.rs: `fcPlcSpaMom` is pair 40 and `fcDggInfo` pair 50. A `PlcSpaMom` entry's
// data is a 26-byte SPA, and `dgglbl` 0 labels the main document's
// OfficeArtWordDrawing. ~keep
const PLC_SPA_MOM_FIB_PAIR: usize = 40;
const DGG_INFO_FIB_PAIR: usize = 50;
const SPA_BYTES: usize = 26;
const MAIN_DOCUMENT_DRAWING: u8 = 0;
/// The raster formats DOC pictures are extracted in.
const PICTURE_FORMATS: &[BlipFormat] = &[BlipFormat::Jpeg, BlipFormat::Png];

pub(crate) struct DocWithImages {
    pub text: DocExtractionResult,
    pub pictures: Vec<DocPicture>,
}

pub(crate) struct DocPicture {
    pub paragraph_index: usize,
    pub text_offset: usize,
    pub standalone: bool,
    pub image: ExtractedImage,
}

/// How a picture character in the main text finds its image.
#[derive(Clone, Copy)]
pub(super) enum PictureReference {
    /// `\x01`: the FC one past the character, keying its CHPX and PICF.
    Inline(u32),
    /// `\x08`: the character's CP, keying its `PlcSpaMom` shape anchor.
    Floating(u32),
}

impl PictureReference {
    /// The reference of the picture character `c`, whose FC end and CP are given. An
    /// inline picture is found through its character properties (FC); a floating shape
    /// anchor through `PlcSpaMom` (CP).
    pub(super) fn new(c: char, fc_end: Option<u32>, cp: Option<u32>) -> Option<Self> {
        match c {
            '\x01' => fc_end.map(Self::Inline),
            '\x08' => cp.map(Self::Floating),
            _ => None,
        }
    }
}

pub(super) struct PicturePosition {
    reference: PictureReference,
    paragraph_index: usize,
    text_offset: usize,
    standalone: bool,
    supported: bool,
}

/// What a picture's OfficeArt records hold.
enum Raster {
    Image(Box<ExtractedImage>),
    /// Malformed records, or a blip in a format other than JPEG and PNG.
    Unsupported,
    /// Well-formed records without any blip.
    Empty,
}

impl From<Blip<'_>> for Raster {
    fn from(blip: Blip<'_>) -> Self {
        match blip {
            Blip::Image(format, bytes) => Self::Image(Box::new(ExtractedImage {
                data: bytes::Bytes::copy_from_slice(bytes),
                format: format.name().into(),
                ..Default::default()
            })),
            Blip::Unsupported => Self::Unsupported,
            Blip::Empty => Self::Empty,
        }
    }
}

pub(super) fn record_position(
    pending: &mut Vec<(PictureReference, usize)>,
    reference: Option<PictureReference>,
    raw_offset: usize,
    limits: Option<&SecurityLimits>,
    count: &mut usize,
) -> Result<()> {
    let Some(limits) = limits else { return Ok(()) };
    // A floating anchor counts once its shape turns out to be a picture: a document can
    // anchor any number of lines and text boxes. ~keep
    if !matches!(reference, Some(PictureReference::Floating(_))) {
        if *count >= limits.max_files_in_archive {
            return Err(XbergError::validation(
                "DOC picture reference count exceeds the configured limit",
            ));
        }
        *count += 1;
    }
    if let Some(reference) = reference {
        pending.push((reference, raw_offset));
    }
    Ok(())
}

pub(super) fn place_pictures(
    pictures: &mut Vec<PicturePosition>,
    pending: &mut Vec<(PictureReference, usize)>,
    raw: &str,
    paragraph_index: usize,
    paragraph: Option<&DocParagraph>,
    supported: bool,
) {
    if pending.is_empty() {
        return;
    }
    map_picture_offsets(raw, pending);
    for (reference, text_offset) in pending.drain(..) {
        pictures.push(PicturePosition {
            reference,
            paragraph_index,
            text_offset,
            standalone: paragraph.is_none(),
            supported,
        });
    }
}

fn map_picture_offsets(raw: &str, pending: &mut [(PictureReference, usize)]) {
    let mut normalized = String::new();
    let mut picture = 0;
    let mut newlines = 0;
    for (offset, character) in raw.char_indices() {
        if pending.get(picture).is_some_and(|(_, position)| *position == offset) {
            pending[picture].1 = normalized.len();
            picture += 1;
        }
        if let Some(character) = super::normalized_doc_character(character) {
            newlines = if character == '\n' { newlines + 1 } else { 0 };
            if newlines <= 2 {
                normalized.push(character);
            }
        }
    }
    let leading = normalized.len() - normalized.trim_start().len();
    let length = normalized.trim().len();
    for (_, offset) in pending {
        *offset = offset.saturating_sub(leading).min(length);
    }
}

pub(super) fn warn_unsupported_fallback(requested: bool, warnings: &mut Vec<ProcessingWarning>, reason: &str) {
    if requested {
        warn(
            warnings,
            &format!("Embedded pictures in DOC {reason} are not extracted"),
        );
    }
}

pub(super) fn warn(warnings: &mut Vec<ProcessingWarning>, message: &str) {
    crate::core::diagnostics::push_warning(warnings, DOC_WARNING_SOURCE, message);
}

fn word(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn dword(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn validation(error: crate::extractors::security::SecurityError) -> XbergError {
    XbergError::validation(error.to_string())
}

pub(super) fn extract_pictures(
    compound: &mut cfb::CompoundFile<Cursor<&[u8]>>,
    word_doc: &[u8],
    table: &[u8],
    positions: &[PicturePosition],
    limits: &SecurityLimits,
    warnings: &mut Vec<ProcessingWarning>,
) -> Result<Vec<DocPicture>> {
    let mut pictures = Vec::new();
    let mut budget = SecurityBudget::from_limits(limits);
    let drawings = Drawings::read(word_doc, table, positions, &mut budget)?;
    for position in positions {
        budget.step().map_err(validation)?;
        let raster = match position.reference {
            PictureReference::Inline(fc_end) => {
                if !position.supported {
                    warn_unsupported_location(warnings);
                    continue;
                }
                let Some(offset) = picture_offset(word_doc, table, fc_end, &mut budget)? else {
                    warn(
                        warnings,
                        "DOC picture has no supported live character-property reference; skipped",
                    );
                    continue;
                };
                check_picture_count(&pictures, limits)?;
                read_picture(compound, offset, limits, &mut budget)?
            }
            PictureReference::Floating(cp) => {
                let Some((drawings, shape)) = drawings
                    .as_ref()
                    .and_then(|drawings| Some((drawings, *drawings.anchors.get(&cp)?)))
                else {
                    warn(warnings, "DOC floating shape has no drawing record; skipped");
                    continue;
                };
                let pib = match shape {
                    Shape::Picture(pib) => pib,
                    Shape::Group => {
                        warn(warnings, "Grouped floating pictures in DOC are not extracted");
                        continue;
                    }
                    // A text box, line or other shape without a picture is not
                    // an image; its text, if any, comes from the text-box story. ~keep
                    Shape::Other => continue,
                };
                if !position.supported {
                    warn_unsupported_location(warnings);
                    continue;
                }
                check_picture_count(&pictures, limits)?;
                drawings.picture(pib, &mut budget)?
            }
        };
        match raster {
            Raster::Image(image) => {
                let mut image = *image;
                image.image_index = pictures.len() as u32;
                pictures.push(DocPicture {
                    paragraph_index: position.paragraph_index,
                    text_offset: position.text_offset,
                    standalone: position.standalone,
                    image,
                });
            }
            Raster::Unsupported => warn(
                warnings,
                "DOC embedded picture is malformed or uses an unsupported image format; skipped",
            ),
            Raster::Empty => warn(warnings, "DOC embedded picture holds no image data; skipped"),
        }
    }
    Ok(pictures)
}

fn warn_unsupported_location(warnings: &mut Vec<ProcessingWarning>) {
    warn(
        warnings,
        "Pictures in DOC table cells or automatic lists are not extracted",
    );
}

fn check_picture_count(pictures: &[DocPicture], limits: &SecurityLimits) -> Result<()> {
    if pictures.len() >= limits.max_files_in_archive {
        return Err(XbergError::validation(
            "DOC embedded picture count exceeds the configured limit",
        ));
    }
    Ok(())
}

/// The table-stream bytes a `FibRgFcLcb97` pair addresses.
fn fib_table<'a>(word_doc: &[u8], table: &'a [u8], pair: usize) -> Option<&'a [u8]> {
    let cslw_offset = 34 + usize::from(word(word_doc, 32)?) * 2;
    let pairs = cslw_offset + 4 + usize::from(word(word_doc, cslw_offset)?) * 4;
    let pair = pairs + pair * 8;
    let start = dword(word_doc, pair)? as usize;
    let length = dword(word_doc, pair + 4)? as usize;
    table.get(start..start.checked_add(length)?)
}

/// The main document's floating-shape anchors and the drawing group's blips.
///
/// [MS-DOC] `PlcSpaMom` maps each anchor CP to a shape id; `OfficeArtContent`
/// at `fcDggInfo` holds the drawing group, whose BStore lists the BSEs
/// (`pib` is a 1-based index into it), then one `dgglbl`-prefixed drawing per
/// document part. Word stores a BSE's BLIP in the WordDocument stream at its
/// `foDelay` ([MS-ODRAW] OfficeArtFBSE) rather than in the BStore. ~keep
struct Drawings<'a> {
    /// The drawing group's blips, which `pib` indexes.
    blips: BlipStore<'a>,
    /// The shape anchored at each main-document CP.
    anchors: HashMap<u32, Shape>,
    /// The WordDocument stream, the delay stream `foDelay` points into.
    word_doc: &'a [u8],
}

impl<'a> Drawings<'a> {
    /// `None` when no picture in `positions` is floating, or the drawings cannot be read.
    fn read(
        word_doc: &'a [u8],
        table: &'a [u8],
        positions: &[PicturePosition],
        budget: &mut SecurityBudget,
    ) -> Result<Option<Self>> {
        if !positions
            .iter()
            .any(|position| matches!(position.reference, PictureReference::Floating(_)))
        {
            return Ok(None);
        }
        let Some(plc) = fib_table(word_doc, table, PLC_SPA_MOM_FIB_PAIR)
            .filter(|plc| plc.len() >= 4 + 4 + SPA_BYTES && (plc.len() - 4) % (4 + SPA_BYTES) == 0)
        else {
            return Ok(None);
        };
        let Some(content) = fib_table(word_doc, table, DGG_INFO_FIB_PAIR) else {
            return Ok(None);
        };
        let Some(group) = record_at(content, 0, content.len()).filter(|record| record.rec_type == MSOFBT_DGG_CONTAINER)
        else {
            return Ok(None);
        };
        let Some(drawing) = main_drawing(content, group.content_end, budget)? else {
            return Ok(None);
        };
        let shapes = drawing_shapes(content, &drawing, budget).map_err(validation)?;
        let count = (plc.len() - 4) / (4 + SPA_BYTES);
        let mut anchors = HashMap::new();
        for index in 0..count {
            budget.step().map_err(validation)?;
            let (Some(cp), Some(spid)) = (dword(plc, index * 4), dword(plc, 4 * (count + 1) + SPA_BYTES * index))
            else {
                continue;
            };
            if let Some(&shape) = shapes.get(&spid) {
                anchors.entry(cp).or_insert(shape);
            }
        }
        Ok(Some(Self {
            blips: BlipStore::from_drawing_group(content, &group),
            anchors,
            word_doc,
        }))
    }

    /// The picture of the blip `pib` names.
    fn picture(&self, pib: u32, budget: &mut SecurityBudget) -> Result<Raster> {
        let blip = self
            .blips
            .blip(pib, self.word_doc, PICTURE_FORMATS, budget)
            .map_err(validation)?;
        // Each anchor gets its own copy of the picture, so each copy counts. ~keep
        if let Blip::Image(_, bytes) = blip {
            budget.account_text(bytes.len()).map_err(validation)?;
        }
        Ok(blip.into())
    }
}

/// The main document's drawing, among the `dgglbl`-prefixed drawings that
/// follow the drawing group from `position` on.
fn main_drawing(content: &[u8], mut position: usize, budget: &mut SecurityBudget) -> Result<Option<ArtRecord>> {
    while let Some(&label) = content.get(position) {
        budget.step().map_err(validation)?;
        let Some(drawing) = record_at(content, position + 1, content.len()) else {
            break;
        };
        if label == MAIN_DOCUMENT_DRAWING && drawing.rec_type == MSOFBT_DG_CONTAINER {
            return Ok(Some(drawing));
        }
        position = drawing.content_end;
    }
    Ok(None)
}

fn picture_offset(word_doc: &[u8], table: &[u8], fc_end: u32, budget: &mut SecurityBudget) -> Result<Option<u32>> {
    let Some(bte) =
        fib_table(word_doc, table, CHPX_FIB_PAIR).filter(|bytes| bytes.len() >= 12 && (bytes.len() - 4) % 8 == 0)
    else {
        return Ok(None);
    };
    let count = (bte.len() - 4) / 8;
    for index in 0..count {
        budget.step().map_err(validation)?;
        if dword(bte, index * 4) >= dword(bte, (index + 1) * 4) {
            return Ok(None);
        }
    }
    for index in 0..count {
        budget.step().map_err(validation)?;
        let (Some(first), Some(last)) = (dword(bte, index * 4), dword(bte, (index + 1) * 4)) else {
            return Ok(None);
        };
        if first >= last {
            return Ok(None);
        }
        if first >= fc_end || fc_end > last {
            continue;
        }
        let Some(page) = dword(bte, 4 * (count + 1 + index)) else {
            break;
        };
        let start = (page & FKP_PAGE_MASK) as usize * FKP_PAGE_BYTES;
        let Some(fkp) = word_doc.get(start..start + FKP_PAGE_BYTES) else {
            continue;
        };
        if let Some(offset) = fkp_picture_offset(fkp, fc_end, budget)? {
            return Ok(Some(offset));
        }
    }
    Ok(None)
}

fn fkp_picture_offset(fkp: &[u8], fc_end: u32, budget: &mut SecurityBudget) -> Result<Option<u32>> {
    let count = usize::from(fkp[FKP_PAGE_BYTES - 1]);
    if 4 * (count + 1) + count > FKP_PAGE_BYTES - 1 {
        return Ok(None);
    }
    for index in 0..count {
        budget.step().map_err(validation)?;
        let (Some(start), Some(end)) = (dword(fkp, index * 4), dword(fkp, (index + 1) * 4)) else {
            break;
        };
        if start >= fc_end || fc_end > end {
            continue;
        }
        let offset = usize::from(fkp[4 * (count + 1) + index]) * 2;
        if offset == 0 {
            return Ok(None);
        }
        let Some(&length) = fkp.get(offset) else {
            return Ok(None);
        };
        let Some(properties) = fkp.get(offset + 1..offset + 1 + usize::from(length)) else {
            return Ok(None);
        };
        return Ok(pic_location(properties));
    }
    Ok(None)
}

fn pic_location(properties: &[u8]) -> Option<u32> {
    let mut position = 0;
    let (mut special, mut binary, mut location) = (false, false, None);
    while let Some(sprm) = word(properties, position) {
        position += 2;
        let length = match sprm >> 13 {
            0 | 1 => 1,
            2 | 4 | 5 => 2,
            3 => 4,
            7 => 3,
            _ => {
                let length = usize::from(*properties.get(position)?);
                position += 1;
                length
            }
        };
        let operand = properties.get(position..position.checked_add(length)?)?;
        match sprm {
            SPRM_C_SPEC => special = operand[0] == 1,
            SPRM_C_DATA => binary = operand[0] != 0,
            SPRM_C_PIC_LOCATION => location = dword(operand, 0).filter(|offset| *offset <= i32::MAX as u32),
            _ => {}
        }
        position += length;
    }
    location.filter(|_| special && !binary && position == properties.len())
}

fn read_picture(
    compound: &mut cfb::CompoundFile<Cursor<&[u8]>>,
    offset: u32,
    limits: &SecurityLimits,
    budget: &mut SecurityBudget,
) -> Result<Raster> {
    let Ok(mut stream) = compound.open_stream("/Data") else {
        return Ok(Raster::Unsupported);
    };
    let Ok(stream_len) = stream.seek(SeekFrom::End(0)) else {
        return Ok(Raster::Unsupported);
    };
    if u64::from(offset) + 6 > stream_len || stream.seek(SeekFrom::Start(u64::from(offset))).is_err() {
        return Ok(Raster::Unsupported);
    }
    let mut header = [0; 6];
    if stream.read_exact(&mut header).is_err() {
        return Ok(Raster::Unsupported);
    }
    let length = dword(&header, 0).unwrap_or(0) as usize;
    let header_length = usize::from(word(&header, 4).unwrap_or(0));
    if header_length < PICF_HEADER_BYTES || length < header_length || u64::from(offset) + length as u64 > stream_len {
        return Ok(Raster::Unsupported);
    }
    if length > limits.max_content_size {
        return Err(XbergError::validation(
            "DOC picture exceeds the configured content limit",
        ));
    }
    budget.account_text(length).map_err(validation)?;
    let mut data = vec![0; length];
    data[..6].copy_from_slice(&header);
    if stream.read_exact(&mut data[6..]).is_err() {
        return Ok(Raster::Unsupported);
    }
    let start = match word(&data, 6) {
        Some(MM_SHAPE) => header_length,
        Some(MM_SHAPEFILE) => match data.get(header_length) {
            Some(length) => header_length + 1 + usize::from(*length),
            None => return Ok(Raster::Unsupported),
        },
        _ => return Ok(Raster::Unsupported),
    };
    let Some(records) = data.get(start..) else {
        return Ok(Raster::Unsupported);
    };
    Ok(find_blip(records, PICTURE_FORMATS, budget).map_err(validation)?.into())
}

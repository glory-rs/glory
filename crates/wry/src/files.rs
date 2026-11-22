//! A WebView document retains the selected File handles. Native callers receive
//! opaque revision-bound tokens; every read is checked against the mapper.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    pub node: String,
    revision: String,
    batch: String,
    request: Option<String>,
    outcome: Outcome,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Outcome {
    Selected { files: Vec<Selected> },
    Cancelled,
    Rejected { reason: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selected {
    serial: String,
    name: String,
    media_type: String,
    size: u64,
}

impl Observation {
    pub(super) fn receive(self, state: &mut State, bytes: usize) -> Result<bool, Error> {
        let Some(node) = state.nodes.id_for_token(&self.node) else {
            return Ok(false);
        };
        let document = state
            .files
            .ok_or(Error::Unsupported("Wry files require host opt-in"))?;
        let revision = protocol::integer(&self.revision)?;
        let outcome = match self.outcome {
            Outcome::Cancelled => FileOutcome::Cancelled,
            Outcome::Rejected { reason } => FileOutcome::Rejected(match reason.as_str() {
                "count" => FileRejection::Count,
                "type" => FileRejection::Type,
                "bytes" => FileRejection::Bytes,
                "metadata" => FileRejection::Metadata,
                "busy" => FileRejection::Busy,
                "unavailable" => FileRejection::Unavailable,
                "failed" => FileRejection::Failed,
                _ => return Err(Error::InvalidEvent("Wry file rejection")),
            }),
            Outcome::Selected { files } => {
                if files.len() > MAX_FILE_COUNT {
                    return Err(Error::InvalidEvent("Wry file count"));
                }
                FileOutcome::Selected(
                    files
                        .into_iter()
                        .map(|file| {
                            Ok(FileInfo {
                                token: FileToken {
                                    document,
                                    node,
                                    revision,
                                    serial: protocol::integer(&file.serial)?,
                                },
                                name: file.name,
                                media_type: file.media_type,
                                size: file.size,
                            })
                        })
                        .collect::<Result<_, Error>>()?,
                )
            }
        };
        let request = self.request.as_deref().map(protocol::integer).transpose()?;
        let event = FileEvent {
            document,
            revision,
            request,
            batch: protocol::integer(&self.batch)?,
            origin: if request.is_some() {
                FileOrigin::Picker
            } else {
                FileOrigin::Drop
            },
            outcome,
        };
        if state.inputs.len().saturating_add(state.pending.len()) >= protocol::MAX_INPUTS {
            return Err(Error::InvalidEvent("Wry input queue budget"));
        }
        let accepted = state.mapper.finish_files(node, document, &event)?;
        if matches!(event.outcome, FileOutcome::Selected(_)) {
            // Preserve the previous authorized handles when a selection and a disable,
            // retirement or program revision cross on their way through IPC.
            state.transport.push(
                json!({"op":"file_result","batch":event.batch.to_string(),"accepted":accepted}),
            )?;
        }
        if !accepted {
            return Ok(false);
        }
        state.inputs.push(Input::Event {
            node,
            event: Event::Files(Box::new(event)),
        });
        state.input_bytes += bytes;
        Ok(true)
    }
}
pub(super) fn action(action: Action, state: &mut State) -> Result<Value, Error> {
    let document = state
        .files
        .ok_or(Error::Unsupported("Wry files require host opt-in"))?;
    let value = match action {
        Action::PickFiles {
            revision, request, ..
        } => {
            json!({"kind":"pick_files","revision":revision.to_string(),"request":request.to_string()})
        }
        Action::ClearFiles { .. } => json!({"kind":"clear_files"}),
        Action::RemoveFile(file) if file.document == document => {
            json!({"kind":"remove_file","serial":file.serial.to_string()})
        }
        _ => return Err(Error::InvalidProps("invalid Wry file action")),
    };
    state.mapper.admit_file_action(action)?;
    Ok(value)
}
pub(super) fn query(query: Query, state: &State) -> Result<Value, Error> {
    let file = match query {
        Query::FileRead(read) => read.file,
        Query::FilePreview(file) => file,
        _ => return Err(Error::Unsupported("Wry file query")),
    };
    if state.files != Some(file.document) {
        return Err(Error::Unsupported("foreign Wry file"));
    }
    if state
        .pending
        .values()
        .filter(|p| matches!(p.query, Query::FileRead(_) | Query::FilePreview(_)))
        .count()
        >= MAX_FILE_READS
    {
        return Err(Error::Unsupported("Wry concurrent file read budget"));
    }
    let selected = state.mapper.tree().selected_file(file)?;
    let (kind, offset, length) = match query {
        Query::FileRead(read) => {
            read.validate_file(selected)?;
            ("file_read", read.offset, u64::from(read.length))
        }
        Query::FilePreview(_) => {
            let Props::File(props) = state.mapper.tree().node(file.node)?.descriptor.view().props
            else {
                unreachable!()
            };
            let limits = props
                .configuration
                .preview
                .ok_or(Error::Unsupported("Wry file preview disabled"))?;
            if selected.size > limits.encoded_bytes {
                return Err(Error::Unsupported(
                    "Wry preview exceeds encoded byte budget",
                ));
            }
            (
                "file_preview",
                0,
                selected.size.min(MAX_FILE_HEADER_BYTES as u64),
            )
        }
        _ => unreachable!(),
    };
    Ok(
        json!({"kind":kind,"serial":file.serial.to_string(),"revision":file.revision.to_string(),"offset":offset,"length":length}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    bytes: Vec<u8>,
}

pub(super) enum Completion {
    Done(QueryValue),
    Preview(FileImageInfo),
}

pub(super) fn response(pending: Pending, value: Value, state: &State) -> Result<Completion, Error> {
    let query = pending.query;
    let result = match query {
        Query::FileRead(read) => {
            read.validate_file(state.mapper.tree().selected_file(read.file)?)?;
            let bytes = bytes(value, read.length as usize)?;
            QueryValue::FileBytes(Box::new(FileChunk {
                file: read.file,
                offset: read.offset,
                bytes,
            }))
        }
        Query::FilePreview(file) => {
            let selected = state.mapper.tree().selected_file(file)?;
            let Props::File(props) = state.mapper.tree().node(file.node)?.descriptor.view().props
            else {
                unreachable!()
            };
            let limits = props
                .configuration
                .preview
                .ok_or(Error::Unsupported("Wry preview disabled"))?;
            if let Some(image) = pending.preview {
                let source = value
                    .as_str()
                    .filter(|v| v.len() <= 2048 && v.starts_with("blob:"))
                    .ok_or(Error::InvalidResponse("invalid Wry preview URI"))?;
                QueryValue::FilePreview(Some(Box::new(FilePreview {
                    file,
                    image,
                    source: source.into(),
                })))
            } else {
                let bytes = bytes(
                    value,
                    selected.size.min(MAX_FILE_HEADER_BYTES as u64) as usize,
                )?;
                if let Ok(image) =
                    inspect_file_preview(&bytes, selected.size, &selected.media_type, limits)
                {
                    return Ok(Completion::Preview(image));
                }
                QueryValue::FilePreview(None)
            }
        }
        _ => return Err(Error::Unsupported("Wry file query")),
    };
    query.validate_response(&result)?;
    Ok(Completion::Done(result))
}

fn bytes(value: Value, length: usize) -> Result<Vec<u8>, Error> {
    if value
        .get("bytes")
        .and_then(Value::as_array)
        .is_none_or(|b| b.len() != length)
    {
        return Err(Error::InvalidResponse("Wry file response length"));
    }
    serde_json::from_value::<Reply>(value)
        .map(|r| r.bytes)
        .map_err(|_| Error::InvalidResponse("invalid Wry file bytes"))
}

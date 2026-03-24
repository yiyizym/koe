/// Events emitted by an ASR provider during streaming recognition.
#[derive(Debug, Clone)]
pub enum AsrEvent {
    /// Connection established successfully.
    Connected,
    /// Interim (partial) recognition result — may change as more audio arrives.
    Interim(String),
    /// A confirmed utterance segment (endpoint detected).
    /// Higher accuracy than Interim as the segment is complete.
    Definite(String),
    /// Final recognition result for the entire session.
    Final(String),
    /// Server-side error message.
    Error(String),
    /// Connection closed.
    Closed,
}

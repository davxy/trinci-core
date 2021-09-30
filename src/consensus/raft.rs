// TODO
// 1. Block header extension: <term, validator, signature>
// 2. Interface for blockchain usage:

use crate::{
    base::serialize::{rmp_deserialize, rmp_serialize},
    crypto::PublicKey,
};

/// Block header consensus data.
#[derive(Serialize, Deserialize)]
pub struct BlockHeaderExt {
    /// Validation term.
    term: u64,
    /// Validator identity.
    validator: PublicKey,
    /// Validator digital signature.
    signature: Vec<u8>,
}

/// Raft message with common header and specialized body.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct RaftMessage {
    term: u64,
    src: String,
    dst: String,
    body: RaftMessageBody,
}

/// Raft message body types.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RaftMessageBody {
    /// Request vote request.
    VoteRequest,
    /// Request vote response.
    VoteResponse(bool),
    AppendEntriesRequest,
    AppendEntriesResponse(bool),
}

pub struct Raft {
    /// Validators list (TODO: the tracer shall eventually update this).
    pub validators: Vec<String>,
    /// Node identifier.
    pub node_id: String,
    /// Latest term this peer has seen (increases monotonically).
    pub term: u64,
    /// Whether this peer belives it is the leader.
    pub is_leader: bool,
    /// Candidate id that received vote in current term (None if we have not voted).
    pub voted_for: Option<String>,

    pub leader_alive: bool,
}

impl Raft {
    pub fn new(node_id: String) -> Self {
        Raft {
            validators: vec![],
            node_id,
            term: 0,
            is_leader: false,
            voted_for: None,
            leader_alive: false,
        }
    }

    pub fn vote_request(&mut self) -> Vec<u8> {
        self.term += 1;
        let req = RaftMessage {
            term: self.term,
            src: self.node_id.clone(),
            dst: "".to_owned(),
            body: RaftMessageBody::VoteRequest,
        };
        warn!("[REQ-TX] {:?} ", req);
        rmp_serialize(&req).unwrap()
    }

    pub fn handle_message(&mut self, buf: Vec<u8>) -> Vec<u8> {
        let msg: RaftMessage = rmp_deserialize(&buf).unwrap();
        warn!("[REQ-RX] {:?}", msg);
        let res_body = match msg.body {
            RaftMessageBody::VoteRequest => {
                if msg.term > self.term || msg.src == self.node_id {
                    self.voted_for = None;
                    self.term = msg.term;
                }
                let granted = if self.voted_for.is_none() {
                    warn!("VOTE GRANTED TO {} !!!", msg.src);
                    self.voted_for = Some(msg.src.clone());
                    true
                } else {
                    false
                };
                Some(RaftMessageBody::VoteResponse(granted))
            }
            RaftMessageBody::VoteResponse(granted) => {
                if msg.term < self.term {
                    return vec![];
                }
                if granted {
                    warn!("VOTE GRANTED FROM {}!!!", msg.src);
                }
                None
            }
            RaftMessageBody::AppendEntriesRequest => None,
            RaftMessageBody::AppendEntriesResponse(_success) => None,
        };
        res_body
            .map(|body| {
                let res = RaftMessage {
                    term: self.term,
                    src: self.node_id.clone(),
                    dst: msg.src.clone(),
                    body,
                };
                warn!("[RES-TX] {:?}", res);
                rmp_serialize(&msg).unwrap()
            })
            .unwrap_or_default()
    }
}

// This file is part of TRINCI.
//
// Copyright (C) 2021 Affidaty Spa.
//
// TRINCI is free software: you can redistribute it and/or modify it under
// the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// TRINCI is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License
// for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with TRINCI. If not, see <https://www.gnu.org/licenses/>.

//! KADEMLIA examples:
//! https://github.com/libp2p/rust-libp2p/blob/master/examples/ipfs-kad.rs
//! https://github.com/libp2p/rust-libp2p/discussions/2177
//! https://github.com/whereistejas/rust-libp2p/blob/4be8fcaf1f954599ff4c4428ab89ac79a9ccd0b9/examples/kademlia-example.rs

use crate::{
    base::serialize::{rmp_deserialize, rmp_serialize},
    blockchain::{message::MultiMessage, BlockRequestSender, Message},
    p2p::truster,
    Error, ErrorKind, Result,
};
use async_std::task;
use futures::{stream::Select, AsyncRead, AsyncWrite, AsyncWriteExt};
use libp2p::{
    core::{
        upgrade::{read_length_prefixed, write_length_prefixed},
        ProtocolName, PublicKey,
    },
    gossipsub::{
        error::PublishError, Gossipsub, GossipsubConfigBuilder, GossipsubEvent, IdentTopic,
        MessageAuthenticity, ValidationMode,
    },
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    kad::{
        record::store::MemoryStore, GetClosestPeersError, GetClosestPeersOk, Kademlia,
        KademliaConfig, KademliaEvent, QueryResult,
    },
    mdns::{Mdns, MdnsConfig, MdnsEvent},
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseConfig,
        RequestResponseEvent, RequestResponseMessage,
    },
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use std::{io, iter, str::FromStr};
use tide::utils::async_trait;

const MAX_TRANSMIT_SIZE: usize = 524288;

// Request-response codec implementation
#[derive(Debug, Clone)]
pub struct TrinciProtocol();
#[derive(Clone)]
pub struct TrinciCodec();
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnicastMessage(pub Vec<u8>);

impl ProtocolName for TrinciProtocol {
    fn protocol_name(&self) -> &[u8] {
        "trinci/1.0.0".as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for TrinciCodec {
    type Protocol = TrinciProtocol;
    type Request = UnicastMessage;
    type Response = UnicastMessage;

    async fn read_request<T>(&mut self, _: &TrinciProtocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, MAX_TRANSMIT_SIZE).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(UnicastMessage(vec))
    }

    async fn read_response<T>(
        &mut self,
        _: &TrinciProtocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, MAX_TRANSMIT_SIZE).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(UnicastMessage(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &TrinciProtocol,
        io: &mut T,
        UnicastMessage(data): UnicastMessage,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &TrinciProtocol,
        io: &mut T,
        UnicastMessage(data): UnicastMessage,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }
}

/// Network behavior for application level message processing.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ComposedEvent")]
pub(crate) struct Behavior {
    /// Peer identification protocol.
    pub identify: Identify,
    /// Gossip-sub as sub/sub protocol.
    pub gossip: Gossipsub,
    /// mDNS for peer discovery.
    pub mdns: Mdns,
    /// Kademlia for peer discovery.
    pub kad: Kademlia<MemoryStore>,
    /// Request-response for unicast protocol.
    pub reqres: RequestResponse<TrinciCodec>,
    /// To forward incoming messages to blockchain service.
    #[behaviour(ignore)]
    pub bc_chan: BlockRequestSender,
    /// List of trusted peers
    #[behaviour(ignore)]
    pub trusted: truster::trusted,
}

#[derive(Debug)]
pub enum ComposedEvent {
    Identify(IdentifyEvent),
    Kademlia(KademliaEvent),
    Gossip(GossipsubEvent),
    Mdns(MdnsEvent),
    ReqRes(RequestResponseEvent<UnicastMessage, UnicastMessage>),
}

impl From<IdentifyEvent> for ComposedEvent {
    fn from(event: IdentifyEvent) -> Self {
        ComposedEvent::Identify(event)
    }
}

impl From<KademliaEvent> for ComposedEvent {
    fn from(event: KademliaEvent) -> Self {
        ComposedEvent::Kademlia(event)
    }
}

impl From<GossipsubEvent> for ComposedEvent {
    fn from(event: GossipsubEvent) -> Self {
        ComposedEvent::Gossip(event)
    }
}

impl From<MdnsEvent> for ComposedEvent {
    fn from(event: MdnsEvent) -> Self {
        ComposedEvent::Mdns(event)
    }
}

impl From<RequestResponseEvent<UnicastMessage, UnicastMessage>> for ComposedEvent {
    fn from(event: RequestResponseEvent<UnicastMessage, UnicastMessage>) -> Self {
        ComposedEvent::ReqRes(event)
    }
}

impl Behavior {
    fn identify_new(public_key: PublicKey) -> Result<Identify> {
        debug!("[p2p] identify start");
        let mut config = IdentifyConfig::new("trinci/1.0.0".to_owned(), public_key);
        config.push_listen_addr_updates = true;
        let identify = Identify::new(config);

        Ok(identify)
    }

    fn mdns_new() -> Result<Mdns> {
        debug!("[p2p] mdns start");
        let fut = Mdns::new(MdnsConfig::default());
        let mdns = task::block_on(fut).map_err(|err| Error::new_ext(ErrorKind::Other, err))?;

        Ok(mdns)
    }

    // TODO: random walk
    // let rand_peer: PeerId = identity::Keypair::generate_ed25519().public().into();
    // kad.get_closest_peers(rand_peer);
    fn kad_new(peer_id: PeerId, bootaddr: Option<String>) -> Result<Kademlia<MemoryStore>> {
        debug!("[p2p] kad start");
        let store = MemoryStore::new(peer_id);
        let config = KademliaConfig::default();
        let mut kad = Kademlia::with_config(peer_id, store, config);

        if let Some(bootaddr) = bootaddr {
            let (boot_peer, boot_addr) = match bootaddr.split_once('@') {
                Some((peer, addr)) => (peer, addr),
                None => {
                    return Err(Error::new_ext(
                        ErrorKind::MalformedData,
                        "address format should be <peer@multiaddr>",
                    ));
                }
            };
            let boot_peer = PeerId::from_str(boot_peer)
                .map_err(|err| Error::new_ext(ErrorKind::MalformedData, err))?;
            let boot_addr = Multiaddr::from_str(boot_addr)
                .map_err(|err| Error::new_ext(ErrorKind::MalformedData, err))?;
            kad.add_address(&boot_peer, boot_addr);

            //kad.bootstrap().unwrap();
            let peer: PeerId = libp2p::identity::Keypair::generate_ed25519()
                .public()
                .into();
            kad.get_closest_peers(peer);
        }

        Ok(kad)
    }

    fn gossip_new(peer_id: PeerId, topic: IdentTopic) -> Result<Gossipsub> {
        debug!("[p2p] gossip start");
        let privacy = MessageAuthenticity::Author(peer_id);
        let gossip_config = GossipsubConfigBuilder::default()
            .validation_mode(ValidationMode::Permissive)
            .max_transmit_size(MAX_TRANSMIT_SIZE)
            .build()
            .map_err(|err| Error::new_ext(ErrorKind::Other, err))?;
        let mut gossip = Gossipsub::new(privacy, gossip_config)
            .map_err(|err| Error::new_ext(ErrorKind::Other, err))?;

        gossip
            .subscribe(&topic)
            .map_err(|err| Error::new_ext(ErrorKind::Other, format!("{:?}", err)))?;

        Ok(gossip)
    }

    fn req_res_new() -> Result<RequestResponse<TrinciCodec>> {
        debug!("[p2p] request-rsponse start");
        let protocols = iter::once((TrinciProtocol(), ProtocolSupport::Full));
        let config = RequestResponseConfig::default();
        let reqres = RequestResponse::new(TrinciCodec(), protocols, config);

        Ok(reqres)
    }

    pub fn new(
        peer_id: PeerId,
        public_key: PublicKey,
        topic: IdentTopic,
        bootaddr: Option<String>,
        bc_chan: BlockRequestSender,
    ) -> Result<Self> {
        let identify = Self::identify_new(public_key)?;
        let gossip = Self::gossip_new(peer_id, topic)?;
        let mdns = Self::mdns_new()?;
        let kad = Self::kad_new(peer_id, bootaddr)?;
        let reqres = Self::req_res_new()?;

        Ok(Behavior {
            identify,
            gossip,
            mdns,
            kad,
            bc_chan,
            reqres,
        })
    }

    /// Populate the trusted peer list
    fn populate_trusted_list() {
        // save starting time
        // until x sec passed:
        //      collect GetBlockResponse
        // select most shared block peers
        todo!()
    }

    ///
    fn ask_block() {
        // ask for block to a trusted
        // if it does not response drop the peer
        // ask to another
        // if no peer do populate_trust_list
    }

    fn ask_tx() {
        // ask for tx to a trusted
        // if it does not response drop the peer
        // ask to another
        // if no peer do populate_trust_list
    }

    // TODO: repopulate trusted each x seconds -> use biusy sleep
}
/* cSpell::disable */
// Received {
//     peer_id: PeerId("12D3KooWFmmKJ7jXhTfoYDvKkPqe7s9pHH42iZdf2xRdM5ykma1p"),
//     info: IdentifyInfo {
//         public_key: Ed25519(PublicKey(compressed): 587b8d516e965a6ee57a19e2734f1ab3bb8b45e6062801dff3e648d8594063),
//         protocol_version: "trinci/1.0.0",
//         agent_version: "rust-libp2p/0.30.0",
//         listen_addrs: [
//             "/ip4/127.0.0.1/tcp/41907",
//             "/ip4/192.168.1.125/tcp/41907",
//             "/ip4/172.18.0.1/tcp/41907",
//             "/ip4/172.17.0.1/tcp/41907"
//         ],
//         protocols: [
//             "/ipfs/id/1.0.0",
//             "/ipfs/id/push/1.0.0",
//             "/meshsub/1.1.0",
//             "/meshsub/1.0.0",
//             "/ipfs/kad/1.0.0"
//         ],
//         observed_addr: "/ip4/192.168.1.116/tcp/54612"
//     }
// }
/* cSpell::enable */

impl NetworkBehaviourEventProcess<IdentifyEvent> for Behavior {
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received { peer_id, info } => {
                // TODO: check protocol, it identyfies the nw: trinci/{hash(bootstrap)/1.0.0}
                self.gossip.add_explicit_peer(&peer_id);
                for addr in info.listen_addrs {
                    info!("[ident] adding {} to kad routing table @ {}", peer_id, addr);
                    self.kad.add_address(&peer_id, addr.clone());
                    info!(
                        "[ident] adding {} to req-res routing table @ {}",
                        peer_id, addr
                    );
                    self.reqres.add_address(&peer_id, addr);
                }
            }
            _ => info!("[ident] event: {:?}", event),
        }
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for Behavior {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(nodes) => {
                for (peer, addr) in nodes {
                    debug!("[mdns] discovered: {} @ {}", peer, addr);
                    self.gossip.add_explicit_peer(&peer);
                    self.reqres.add_address(&peer, addr);
                }
            }
            MdnsEvent::Expired(nodes) => {
                for (peer, addr) in nodes {
                    debug!("[mdns] expired: {} @ {}", peer, addr);
                    self.gossip.remove_explicit_peer(&peer);
                    self.reqres.remove_address(&peer, &addr);
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for Behavior {
    fn inject_event(&mut self, event: KademliaEvent) {
        #[allow(clippy::match_single_binding)]
        match event {
            KademliaEvent::RoutingUpdated {
                peer, addresses, ..
            } => {
                for addr in addresses.iter() {
                    debug!("[kad] discovered: {} @ {}", peer, addr);
                }
                // probably it is not needed, actually it may be a problelm
                // we add the peer already in identify event
                //self.gossip.add_explicit_peer(&peer);
            }
            KademliaEvent::OutboundQueryCompleted {
                result: QueryResult::GetClosestPeers(result),
                ..
            } => {
                let peers = match result {
                    Ok(GetClosestPeersOk { peers, .. }) => peers,
                    Err(GetClosestPeersError::Timeout { peers, .. }) => peers,
                };
                self.identify.push(peers);
            }
            _ => {
                debug!("[kad] event: {:?}", event);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for Behavior {
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Message {
                propagation_source: _,
                message,
                message_id: _,
            } => {
                // TOCHECK
                match self
                    .bc_chan
                    .send_sync(Message::Packed { buf: message.data })
                {
                    Ok(res_chan) => {
                        // Check if the blockchain has a response or if has dropped the response channel.
                        if let Ok(Message::Packed { buf }) = res_chan.recv_sync() {
                            let topic = IdentTopic::new(message.topic.as_str());
                            // TODO: maybe this should be sent in unicast only to the sender.
                            // it should only answer to requests, if block or tx annunciation, just submit to bc service
                            if let Err(err) = self.gossip.publish(topic, buf) {
                                if !matches!(err, PublishError::InsufficientPeers) {
                                    error!("publish error: {:?}", err);
                                }
                            }
                        }
                    }
                    Err(_err) => {
                        warn!("blockchain service seems down");
                    }
                }
            }
            GossipsubEvent::Subscribed { peer_id, topic } => {
                debug!("[pubsub] subscribed peer-id: {}, topic: {}", peer_id, topic);
                // Sending local last block to new comers in unicast
                let msg = Message::GetBlockRequest {
                    height: u64::MAX,
                    txs: false,
                };
                match self.bc_chan.send_sync(msg) {
                    Ok(res_chan) => {
                        if let Ok(Message::Packed { buf }) = res_chan.recv_sync() {
                            let request_id =
                                self.reqres.send_request(&peer_id, UnicastMessage(buf));
                            debug!(
                                "[req-res] message (request) {} containing last block sended to {}",
                                request_id.to_string(),
                                peer_id.to_string()
                            );
                        }
                    }
                    Err(_err) => {
                        warn!("blockchain service seems down");
                    }
                }
            }
            GossipsubEvent::Unsubscribed { peer_id, topic } => {
                debug!(
                    "[pubsub] unsubscribed peer-id: {}, topic: {}",
                    peer_id, topic
                );
            }
            GossipsubEvent::GossipsubNotSupported { peer_id } => {
                debug!("[pubsub] peer-id: {} don't support pubsub", peer_id);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<RequestResponseEvent<UnicastMessage, UnicastMessage>>
    for Behavior
{
    fn inject_event(&mut self, event: RequestResponseEvent<UnicastMessage, UnicastMessage>) {
        match event {
            RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Request {
                        request: UnicastMessage(buf),
                        request_id,
                        channel,
                    },
            } => {
                debug!("[req-res](req) message recieved from: {}", peer.to_string());
                // the message recieved is incapsulated in Message::Packed
                let msg = Message::Packed { buf: buf.clone() };
                // chech wether is:
                //  - GetTransactionRequest
                //  - GetBlockRequest
                //  - GetBlockResponse
                // In case of *Request => ask localy to blockchain service and sand back a reqres.response.
                // In case of GetBlockResponse => submit last block to local blockchain service.
                match rmp_deserialize(&buf) {
                    Ok(MultiMessage::Simple(req)) => match req {
                        Message::GetTransactionRequest { hash: _ } => {
                            match self.bc_chan.send_sync(msg) {
                                Ok(res_chan) => {
                                    if let Ok(Message::Packed { buf }) = res_chan.recv_sync() {
                                        if self
                                            .reqres
                                            .send_response(channel, UnicastMessage(buf))
                                            .is_ok()
                                        {
                                            debug!(
                                                "[req-res] message (response) {} containing tx sended",
                                                request_id.to_string(),
                                            );
                                        } else {
                                            debug!(
                                                "[req-res] message (response) {} error, caused by TO or unreachable peer",
                                                request_id.to_string(),
                                            );
                                        }
                                    }
                                }
                                Err(_err) => {
                                    warn!("blockchain service seems down");
                                }
                            }
                        }
                        Message::GetBlockRequest { height: _, txs: _ } => {
                            match self.bc_chan.send_sync(msg) {
                                Ok(res_chan) => {
                                    if let Ok(Message::Packed { buf }) = res_chan.recv_sync() {
                                        if self
                                            .reqres
                                            .send_response(channel, UnicastMessage(buf))
                                            .is_ok()
                                        {
                                            debug!(
                                                "[req-res] message (response) {} containing block sended",
                                                request_id.to_string(),
                                            );
                                        } else {
                                            debug!(
                                                "[req-res] message (response) {} error, caused by TO or unreachable peer",
                                                request_id.to_string(),
                                            );
                                        }
                                    }
                                }
                                Err(_err) => {
                                    warn!("blockchain service seems down");
                                }
                            }
                        }
                        Message::GetBlockResponse { block: _, txs: _ } => {
                            // this is only possible when peer just subscribed to new topic
                            // collect blocks and peer id in a time window
                            // select a group of trusted peers
                            // submit block to blockchain service
                            // from now on if it has to ask for blocks and block it asks to the trusted peers
                            match self.bc_chan.send_sync(msg) {
                                Ok(res_chan) => match res_chan.recv_sync() {
                                    Ok(_) => debug!(
                                        "[req-res](req) block submited to blockchain service"
                                    ),
                                    Err(error) => {
                                        debug!("[req-res](req) error in submitig block: {}", error)
                                    }
                                },
                                Err(_err) => {
                                    warn!("blockchain service seems down");
                                }
                            }
                        }
                        _ => error!("[reqres](req) unexpected blockchain message"),
                    },
                    _ => error!("[reqres](req) unexpected blockchain message"),
                };
            }
            RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Response {
                        response: UnicastMessage(buf),
                        ..
                    },
            } => {
                // In this scenario only response from blockchain are expected:
                //  - GetBlockResponse
                //  - GetTransactionResponse
                // this means that it only needed to submit it to blockchain service.
                debug!(
                    "[req-res](res) message  recieved from: {}",
                    peer.to_string()
                );

                let msg = Message::Packed { buf };
                // sending response message into th blockchain service
                match self.bc_chan.send_sync(msg) {
                    Ok(res_chan) => {
                        match res_chan.recv_sync() {
                            Ok(_) => {
                                debug!("[req-res](res) recieved response submitted to blockchain service")
                            }
                            Err(error) => {
                                debug!("[req-res](res) error in submitig response: {}", error)
                            }
                        }
                    }
                    Err(_err) => {
                        warn!("blockchain service seems down");
                    }
                }
            }
            RequestResponseEvent::OutboundFailure {
                peer,
                request_id,
                error,
            } => {
                debug!(
                    "[req-res] {} failed {}: {}",
                    peer.to_string(),
                    request_id.to_string(),
                    error.to_string()
                );
            }
            RequestResponseEvent::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                debug!(
                    "[req-res] failed {} for {} : {}",
                    request_id.to_string(),
                    peer.to_string(),
                    error.to_string()
                );
            }
            RequestResponseEvent::ResponseSent { peer, request_id } => {
                debug!(
                    "[req-res] response sent to {} for request {}",
                    peer.to_string(),
                    request_id.to_string()
                );
            }
        }
    }
}

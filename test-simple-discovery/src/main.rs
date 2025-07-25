use anyhow::Result;
use futures::StreamExt;
use libp2p::{
    gossipsub::{
        self,
        Behaviour as GossipsubBehaviour,
        Event as GossipsubEvent,
        IdentTopic as Topic,
        MessageAuthenticity,
    },
    identity::Keypair,
    multiaddr::Protocol,
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::env;
use tokio::time::{interval, Duration};

fn add_explicit_from_addr(swarm: &mut Swarm<GossipsubBehaviour>, addr: &Multiaddr) {
    let mut maybe_peer: Option<PeerId> = None;
    for p in addr.iter() {
        if let Protocol::P2p(peer) = p {
            maybe_peer = Some(peer);
        }
    }
    if let Some(peer) = maybe_peer {
        swarm.behaviour_mut().add_explicit_peer(&peer);
        println!("add explicit peer: {peer}");
    } else {
        eprintln!("can't add explicit peer");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut swarm: Swarm<GossipsubBehaviour> = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default().nodelay(true),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_dns()?
        .with_behaviour(|kp: &Keypair| {
            let cfg = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .build()
                .unwrap();
            GossipsubBehaviour::new(MessageAuthenticity::Signed(kp.clone()), cfg)
                .expect("gossipsub behaviour")
        })?
        .build();

    let local_peer_id = *swarm.local_peer_id();
    println!("Local peer id: {local_peer_id}");

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    if let Some(addr_str) = env::args().nth(1) {
        let addr: Multiaddr = addr_str.parse()?;
        println!("Dialing {addr}");
        add_explicit_from_addr(&mut swarm, &addr);
        swarm.dial(addr)?;
    }

    let topic = Topic::new("demo");
    swarm.behaviour_mut().subscribe(&topic)?;

    let topic_hash = topic.hash();
    let mut tick = interval(Duration::from_secs(2));
    let mut n = 0u64;

    loop {
        tokio::select! {
            _ = tick.tick() => {
                if swarm.behaviour().mesh_peers(&topic_hash).next().is_none() {
                    continue;
                }
                let msg = format!("hello #{n} from {local_peer_id}");
                n += 1;
                if let Err(e) = swarm.behaviour_mut().publish(topic.clone(), msg.into_bytes()) {
                    eprintln!("publish error: {e}");
                }
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Connection string: {address}/p2p/{local_peer_id}");
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        println!("ConnectionEstablished to {peer_id} via {endpoint:?}");
                    }
                    SwarmEvent::Behaviour(GossipsubEvent::Subscribed { peer_id, topic }) => {
                        println!("{peer_id} subscribed to {topic}");
                    }
                    SwarmEvent::Behaviour(GossipsubEvent::Unsubscribed { peer_id, topic }) => {
                        println!("{peer_id} unsubscribed from {topic}");
                    }
                    SwarmEvent::Behaviour(GossipsubEvent::Message { propagation_source, message, .. }) => {
                        println!("got from {propagation_source}: {}", String::from_utf8_lossy(&message.data));
                    }
                    _ => {}
                }
            }
        }
    }
}

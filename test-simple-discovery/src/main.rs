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
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::env;
use tokio::time::{interval, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let mut swarm: Swarm<GossipsubBehaviour> = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_quic()
        .with_dns()?
        .with_behaviour(|kp: &Keypair| {
            let cfg = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .build()
                .expect("valid config");

            GossipsubBehaviour::new(MessageAuthenticity::Signed(kp.clone()), cfg)
                .expect("gossipsub behaviour")
        })?
        .build();

    let local_peer_id: PeerId = *swarm.local_peer_id();
    println!("Local peer id: {local_peer_id}");

    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;

    if let Some(addr) = env::args().nth(1) {
        let addr: Multiaddr = addr.parse()?;
        println!("Dialing {addr}");
        swarm.dial(addr)?;
    }

    let topic = Topic::new("demo");
    swarm.behaviour_mut().subscribe(&topic)?;

    let mut tick = interval(Duration::from_secs(2));
    let mut n = 0u64;

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let msg = format!("hello numbr #{n} from {local_peer_id}");
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
                    SwarmEvent::Behaviour(GossipsubEvent::Message { propagation_source, message, .. }) => {
                        println!("got from {propagation_source}: {}", String::from_utf8_lossy(&message.data));
                    }
                    _ => {}
                }
            }
        }
    }
}

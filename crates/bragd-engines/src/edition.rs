//! The Edition of a running engine: what it says it is, the network it was given or its image
//! was built with, and the image's digest (`bragd_positions::Edition`).

use bragd_positions::Edition;

use crate::network;
use crate::protocol::About;
use crate::session::Program;

/// Networks built into an image, by Engine key: the SHA-256 its Dockerfile checks
/// (`packaging/images/<key>/Dockerfile`).
const BUILT_IN: &[(&str, &str)] = &[("katago", "09456899f1b9155217f4ca059cf8a68f79b7eba22cf6b7c00ffb7f17ce067967")];

/// The Edition of the engine `engine` (its `known::Known::key`, or its program's name) that
/// greeted Bragd with `about`, run as `program`.
// shortcut: an installed engine's own network is not hashed, so builds of one version that
// differ only in their network are one Edition; read its EvalFile/WeightsFile and hash it when
// installed engines join shared jobs (Peers, 0.5.0).
pub fn of(engine: &str, about: &About, program: &Program) -> Edition {
    let version = if about.name.trim().is_empty() { engine.to_string() } else { about.name.trim().to_string() };
    let (network, image) = match program {
        Program::Installed { .. } => (None, None),
        Program::Container { image, network: given, .. } => {
            let net = match given {
                Some(_) => network::for_engine(engine).map(|n| n.sha256),
                None => BUILT_IN.iter().find(|b| b.0 == engine).map(|b| b.1),
            };
            // Only a pinned image (`name@sha256:…`) has a digest; one built here has none.
            (net.map(String::from), image.split_once('@').map(|(_, d)| d.to_string()))
        }
    };
    Edition { engine: engine.into(), version, network, image }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::known;
    use crate::tools::Runtime;
    use std::path::PathBuf;

    fn container(key: &str, network: bool) -> Program {
        let k = known::known(key).unwrap();
        Program::Container { rt: Runtime { name: "podman", path: PathBuf::from("podman") }, image: k.image().unwrap(), args: vec![], syzygy: None, network: network.then(|| PathBuf::from("/n")) }
    }

    fn about(name: &str) -> About {
        About { name: name.into(), ..About::default() }
    }

    #[test]
    fn a_pinned_image_names_its_digest_and_its_network() {
        let e = of("pikafish", &about("Pikafish 2026-09-06"), &container("pikafish", true));
        assert_eq!(e.version, "Pikafish 2026-09-06");
        assert_eq!(e.network.as_deref(), Some(network::PIKAFISH.sha256));
        assert_eq!(e.image.as_deref(), known::PIN_PIKAFISH);
        let lc0 = of("lc0", &about("Lc0 v0.32.1"), &container("lc0", true));
        assert_eq!(lc0.network.as_deref(), Some(network::LC0.sha256));
        let sf = of("stockfish", &about("Stockfish 17.1"), &container("stockfish", false));
        assert_eq!((sf.network, sf.image.as_deref()), (None, known::PIN_STOCKFISH));
    }

    #[test]
    fn katago_names_the_network_its_image_is_built_with() {
        let e = of("katago", &about("KataGo 1.18.2"), &container("katago", false));
        let net = e.network.unwrap();
        assert!(include_str!("../../../packaging/images/katago/Dockerfile").contains(&net), "BUILT_IN is the Dockerfile's network");
    }

    #[test]
    fn an_installed_or_unpinned_engine_has_no_digest() {
        let installed = Program::Installed { path: PathBuf::from("/usr/bin/stockfish"), args: vec![] };
        let e = of("stockfish", &about(" Stockfish 17.1 "), &installed);
        assert_eq!(e, Edition { engine: "stockfish".into(), version: "Stockfish 17.1".into(), network: None, image: None });
        let dev = Program::Container { rt: Runtime { name: "podman", path: PathBuf::from("podman") }, image: "localhost/bragd-stockfish:dev".into(), args: vec![], syzygy: None, network: None };
        assert_eq!(of("stockfish", &about(""), &dev).image, None);
        assert_eq!(of("stockfish", &about(""), &dev).version, "stockfish");
        // A new image is a new Edition, though the version is the same.
        assert_ne!(of("stockfish", &about("Stockfish 17.1"), &container("stockfish", false)).id(), e.id());
    }
}

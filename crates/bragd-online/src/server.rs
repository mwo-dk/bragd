//! The servers Bragd plays on, and the Game families each one plays.

/// A server: its name in texts, its address, and the variables that point Bragd at a fake one.
#[derive(Debug, PartialEq, Eq)]
pub struct Server {
    /// `lichess`: its key in commands, the page and F1.
    pub id: &'static str,
    /// `Lichess`, as texts name it.
    pub name: &'static str,
    /// `lichess.org`, as a link names it.
    pub site: &'static str,
    /// The real server.
    pub url: &'static str,
    /// `BRAGD_LICHESS_URL` points Bragd at another server, such as a fake one on this machine.
    pub url_var: &'static str,
    /// `BRAGD_LICHESS_TOKEN` stands in for the keychain, only for a server on this machine (the
    /// screenshots' fake, whose sandbox has no keychain).
    pub token_var: &'static str,
    /// Its terms, linked beside the chat.
    pub terms: &'static str,
    /// The Game families it plays that Bragd has rules for.
    pub families: &'static [&'static str],
}

pub const LICHESS: Server = Server {
    id: "lichess",
    name: "Lichess",
    site: "lichess.org",
    url: "https://lichess.org",
    url_var: "BRAGD_LICHESS_URL",
    token_var: "BRAGD_LICHESS_TOKEN",
    terms: "https://lichess.org/terms-of-service",
    families: &["chess"],
};

pub const LISHOGI: Server = Server {
    id: "lishogi",
    name: "Lishogi",
    site: "lishogi.org",
    url: "https://lishogi.org",
    url_var: "BRAGD_LISHOGI_URL",
    token_var: "BRAGD_LISHOGI_TOKEN",
    terms: "https://lishogi.org/terms-of-service",
    families: &["shogi"],
};

pub const OGS: Server = Server {
    id: "ogs",
    name: "OGS",
    site: "online-go.com",
    url: "https://online-go.com",
    url_var: "BRAGD_OGS_URL",
    token_var: "BRAGD_OGS_TOKEN",
    terms: "https://online-go.com/terms-of-service",
    families: &["go"],
};

/// Every server Bragd plays on.
pub const ALL: [&Server; 3] = [&LICHESS, &LISHOGI, &OGS];

pub fn find(id: &str) -> Option<&'static Server> {
    ALL.into_iter().find(|s| s.id == id)
}

impl Server {
    /// The address Bragd plays at: the real one, or the one its variable names.
    pub fn base(&self) -> String {
        std::env::var(self.url_var).ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| self.url.to_string()).trim_end_matches('/').to_string()
    }

    /// The token kept for `base`: the keychain's, or the variable's for a server on this machine.
    pub fn token(&self, base: &str) -> Option<String> {
        let fake = std::env::var(self.token_var).ok().filter(|_| crate::board::on_this_machine(base));
        fake.or_else(|| bragd_model::key::get_in(crate::session::SERVICE, base))
    }

    /// Plays a Family that is on.
    pub fn plays_any(&self, on: impl Fn(&str) -> bool) -> bool {
        self.families.iter().any(|f| on(f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_each_has_its_own_variables_and_families() {
        let vars: std::collections::HashSet<_> = ALL.iter().flat_map(|s| [s.url_var, s.token_var]).collect();
        assert_eq!(vars.len(), ALL.len() * 2);
        assert_eq!(find("lishogi").unwrap().families, ["shogi"]);
        assert_eq!(find("ogs").unwrap().families, ["go"]);
        assert!(find("pychess").is_none(), "no API a client may play through (docs/online/pychess.md)");
        assert!(LISHOGI.plays_any(|f| f == "shogi") && !LISHOGI.plays_any(|f| f == "chess"));
    }
}

const SERVERS: [(u64, &str); 4] = [
    (1456204440951455786, "Hagane"),
    (348741070990868481, "The Battle Cats Wiki Official Discord"),
    (1555615248185036820, "Akane Tounji Fanclub"),
    (1421463076594651238, "Temmoz Tier-List"),
];

const USERS: [(u64, &str); 1] = [(1360862647578394704, "@omochikaeri15")];

pub(crate) fn server(id: u64) -> Option<&'static str> {
    SERVERS.iter().find(|(listed, _)| *listed == id).map(|(_, name)| *name)
}

pub(crate) fn user(id: u64) -> Option<&'static str> {
    USERS.iter().find(|(listed, _)| *listed == id).map(|(_, name)| *name)
}

pub(crate) fn admits(server_id: u64) -> bool {
    server(server_id).is_some()
}

pub(crate) fn trusts(user_id: u64) -> bool {
    user(user_id).is_some()
}

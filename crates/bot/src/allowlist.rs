const SERVERS: [u64; 2] = [
    1456204440951455786, // Hagane
    348741070990868481, // The Battle Cats Wiki Official Discord
];

const USERS: [u64; 1] = [
    1360862647578394704, // @omochikaeri15
];

pub(crate) fn admits(server: u64) -> bool {
    SERVERS.contains(&server)
}

pub(crate) fn trusts(user: u64) -> bool {
    USERS.contains(&user)
}

const SERVERS: [u64; 3] = [
    1456204440951455786, // Hagane
    348741070990868481, // The Battle Cats Wiki Official Discord
    1555615248185036820, // Akane Tounji Fanclub
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

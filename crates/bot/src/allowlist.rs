const SERVERS: [u64; 2] = [
    1456204440951455786, // Hagane
    348741070990868481, // The Battle Cats Wiki Official Discord
];

pub(crate) fn admits(server: u64) -> bool {
    SERVERS.contains(&server)
}

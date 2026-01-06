use builder_derive::Builder;

#[derive(Builder, Debug)]
struct Profile {
    username: String,
    email: String,
    bio: Option<String>,
    website: Option<String>,
    age: Option<u32>,
}

fn main() {
    println!("=== Optional Fields Example ===\n");

    // Build with only required fields
    let minimal = Profile::builder()
        .username("alice".to_string())
        .email("alice@example.com".to_string())
        .build()
        .expect("Failed to build minimal profile");

    println!("Minimal profile: {:?}", minimal);

    // Build with some optional fields
    let partial = Profile::builder()
        .username("bob".to_string())
        .email("bob@example.com".to_string())
        .bio("Rust enthusiast".to_string())
        .age(25)
        .build()
        .expect("Failed to build partial profile");

    println!("\nPartial profile: {:?}", partial);

    // Build with all fields
    let complete = Profile::builder()
        .username("charlie".to_string())
        .email("charlie@example.com".to_string())
        .bio("Full-stack developer".to_string())
        .website("https://example.com".to_string())
        .age(30)
        .build()
        .expect("Failed to build complete profile");

    println!("\nComplete profile: {:?}", complete);
}

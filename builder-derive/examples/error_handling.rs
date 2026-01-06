use builder_derive::Builder;

#[derive(Builder, Debug)]
struct Registration {
    username: String,
    email: String,
    password: String,
    terms_accepted: bool,
}

fn main() {
    println!("=== Error Handling Example ===\n");

    // Successful registration
    println!("1. Successful registration:");
    match Registration::builder()
        .username("alice".to_string())
        .email("alice@example.com".to_string())
        .password("secret123".to_string())
        .terms_accepted(true)
        .build()
    {
        Ok(reg) => println!("   Success: {:?}\n", reg),
        Err(e) => println!("   Error: {}\n", e),
    }

    // Missing username
    println!("2. Missing username:");
    match Registration::builder()
        .email("bob@example.com".to_string())
        .password("secret456".to_string())
        .terms_accepted(true)
        .build()
    {
        Ok(reg) => println!("   Success: {:?}\n", reg),
        Err(e) => println!("   Error: {}\n", e),
    }

    // Missing email
    println!("3. Missing email:");
    match Registration::builder()
        .username("charlie".to_string())
        .password("secret789".to_string())
        .terms_accepted(true)
        .build()
    {
        Ok(reg) => println!("   Success: {:?}\n", reg),
        Err(e) => println!("   Error: {}\n", e),
    }

    // Missing multiple fields
    println!("4. Missing multiple fields:");
    match Registration::builder().username("dave".to_string()).build() {
        Ok(reg) => println!("   Success: {:?}\n", reg),
        Err(e) => println!("   Error: {}\n", e),
    }

    // Using Result propagation with ?
    println!("5. Using Result propagation:");
    let result = create_registration();
    match result {
        Ok(reg) => println!("   Created registration: {:?}\n", reg),
        Err(e) => println!("   Failed: {}\n", e),
    }
}

fn create_registration() -> Result<Registration, String> {
    Registration::builder()
        .username("eve".to_string())
        .email("eve@example.com".to_string())
        .password("secret000".to_string())
        .terms_accepted(true)
        .build()
}

use builder_derive::Builder;

#[derive(Builder, Debug, PartialEq)]
struct User {
    username: String,
    email: String,
    age: Option<u32>,
}

#[test]
fn test_builder_all_fields_set() {
    let user = User::builder()
        .username("alice".to_string())
        .email("alice@example.com".to_string())
        .age(30)
        .build();

    assert!(user.is_ok());
    let user = user.unwrap();
    assert_eq!(user.username, "alice");
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.age, Some(30));
}

#[test]
fn test_builder_optional_field_omitted() {
    let user = User::builder()
        .username("bob".to_string())
        .email("bob@example.com".to_string())
        .build();

    assert!(user.is_ok());
    let user = user.unwrap();
    assert_eq!(user.username, "bob");
    assert_eq!(user.email, "bob@example.com");
    assert_eq!(user.age, None);
}

#[test]
fn test_builder_missing_required_field() {
    let result = User::builder().username("charlie".to_string()).build();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("email is required"));
}

#[test]
fn test_builder_method_chaining() {
    let user = User::builder()
        .username("dave".to_string())
        .email("dave@example.com".to_string())
        .age(25)
        .build()
        .unwrap();

    assert_eq!(user.username, "dave");
    assert_eq!(user.email, "dave@example.com");
    assert_eq!(user.age, Some(25));
}

#[derive(Builder, Debug, PartialEq)]
struct Config {
    host: String,
    port: u16,
    timeout: Option<u64>,
    features: Vec<String>,
}

#[test]
fn test_builder_with_vec_field_set() {
    let config = Config::builder()
        .host("localhost".to_string())
        .port(8080)
        .features(vec!["feature1".to_string(), "feature2".to_string()])
        .build()
        .unwrap();

    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 8080);
    assert_eq!(config.features, vec!["feature1", "feature2"]);
}

#[test]
fn test_builder_with_vec_field_omitted() {
    let config = Config::builder()
        .host("localhost".to_string())
        .port(8080)
        .build()
        .unwrap();

    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 8080);
    assert_eq!(config.features, Vec::<String>::new());
}

#[derive(Builder, Debug)]
struct ComplexStruct {
    name: String,
    count: i32,
    optional_string: Option<String>,
    optional_number: Option<f64>,
    tags: Vec<String>,
    flags: Vec<bool>,
}

#[test]
fn test_builder_complex_struct() {
    let obj = ComplexStruct::builder()
        .name("test".to_string())
        .count(42)
        .optional_string("hello".to_string())
        .tags(vec!["tag1".to_string()])
        .build()
        .unwrap();

    assert_eq!(obj.name, "test");
    assert_eq!(obj.count, 42);
    assert_eq!(obj.optional_string, Some("hello".to_string()));
    assert_eq!(obj.optional_number, None);
    assert_eq!(obj.tags, vec!["tag1"]);
    assert_eq!(obj.flags, Vec::<bool>::new());
}

#[derive(Builder)]
pub struct PublicStruct {
    pub field: String,
}

#[test]
fn test_builder_visibility() {
    let obj = PublicStruct::builder()
        .field("test".to_string())
        .build()
        .unwrap();

    assert_eq!(obj.field, "test");
}

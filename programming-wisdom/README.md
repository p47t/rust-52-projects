This is a AWS Lambda function in Rust for an Alexa Skill.

### Usage

- Build it with x86_64-unknown-linux-musl toolchain:
  - `cargo build --release --target x86_64-unknown-linux-musl`
- Create deployment package:
  - $ **cp ../target/x86_64-unknown-linux-musl/release/programming-wisdom ./bootstrap && zip lambda.zip bootstrap && rm bootstrap**
- Create the Lambda function with AWS `awscli`:
  - $ **aws lambda create-function --function-name programmingWisdom --handler doesnt.matter --runtime provided --role** _your_role_ **--zip-file fileb://./lambda.zip**
- Invoke the function:
  - **aws lambda invoke --function-name programmingWisdom output.json**

### Things I Learned

- How to create and deploy a AWS Lambda function.
- How to create an Alexa Skill to trigger a Lambda function.
- Use `serde` to serialize and deserialize Rust struct from/to JSON.

### Reference

- [Request and Response JSON Reference | Alexa Skills Kit](https://developer.amazon.com/en-US/docs/alexa/custom-skills/request-and-response-json-reference.html)
- [JSON to Rust Serde](https://transform.tools/json-to-rust-serde)
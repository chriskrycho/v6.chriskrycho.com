pub fn example() -> Result<String, String> {
    let parse_options = markdown::ParseOptions {
        constructs: markdown::Constructs {
            frontmatter: true,
            math_flow: true,
            math_text: true,
            ..markdown::Constructs::gfm()
        },
        ..markdown::ParseOptions::gfm()
    };

    let test_content = r#"    
Now: a *paragraph*, yo! $x + y = 12$ and a [link][to-here].

```js
let foo = true;
```

[to-here]: https://www.example.com "the usual"

$$
\frac{1}{2}
$$

COOL.[^really]

[^really]: Really? Really.

> Really, really, really even.

Or so I have been told.
"#;

    match markdown::to_mdast(test_content, &parse_options) {
        Ok(ast) => Ok(format!("{ast:?}")),
        Err(reason) => Err(reason),
    }
}

use self::mdast_ext::Transforms;

mod mdast_ext;

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

   markdown::to_mdast(test_content, &parse_options).map(|ast| {
      let mut buffer = String::with_capacity(test_content.len() * 2);
      mdast_ext::ast_to_html(&ast, &mut buffer, &Transforms::default());
      buffer
   })
}

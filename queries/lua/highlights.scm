(function_declaration
  name: (identifier) @function)

(local_function
  name: (identifier) @function)

(function_call
  name: (identifier) @function)

(identifier) @variable

(number) @number

(string) @string

(comment) @comment

(keyword) @keyword

[
  "local"
  "require"
  "return"
  "if"
  "then"
  "else"
  "elseif"
  "end"
  "for"
  "while"
  "repeat"
  "until"
  "do"
  "function"
] @keyword

[
  "="
  "+"
  "-"
  "*"
  "/"
  "%"
  "^"
  "=="
  "~="
  "<"
  ">"
  "<="
  ">="
  "and"
  "or"
  "not"
] @operator

[
  "{"
  "}"
  "["
  "]"
  "("
  ")"
] @punctuation

(table_constructor
  "{" @punctuation
  "}" @punctuation)

(table_index
  "." @operator)
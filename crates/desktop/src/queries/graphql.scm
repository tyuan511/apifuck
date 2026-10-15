(comment) @comment
(operation_type) @keyword
["fragment" "on"] @keyword
(operation_definition (name) @function)
(fragment_name (name) @function)
(field (name) @property)
(alias (name) @property)
(argument (name) @property)
(variable) @variable.special
(named_type (name) @type)
(directive (name) @attribute)
(string_value) @string
[(int_value) (float_value)] @number
(boolean_value) @boolean
(null_value) @constant
(enum_value (name) @constant)
["!" "=" "@" "..."] @operator
["(" ")" "{" "}" "[" "]"] @punctuation.bracket
":" @punctuation.delimiter

extends Node


func flat(a: bool) -> int:
	return 1 if a else 2


func chained(a: bool, b: bool) -> int:
	return 1 if a else 2 if b else 3


func wrapped(a: bool, b: bool) -> int:
	return (1 if a else 2) if b else 3


func in_condition(a: bool, b: bool) -> int:
	return 1 if (a if b else false) else 2


func pick(a: bool, b: bool) -> int:
	return flat(a) + chained(a, b) + wrapped(a, b) + in_condition(a, b)

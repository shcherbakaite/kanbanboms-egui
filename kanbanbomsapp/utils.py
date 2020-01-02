import re

def normalize_partno(partno):
	regex = re.compile('\W*([0-9]{5})\s*-\s*([a-zA-Z]{2})\s*-\s*([0-9]{3})\s*')
	regex2 = re.compile('\W*EL\s*-\s*([0-9]{4})\s*')
	match = regex.match(partno.upper())
	match2= regex2.match(partno.upper())
	if match:
		return match.group(1) + "-" + match.group(2) + "-" + match.group(3)
	elif match2:
		return "EL-" + match2.group(1);
	else:
		return ""

# def normalize_partno(partno):
# 	regex = re.compile('\s*([0-9]{5})\s*-\s*([a-zA-Z]{2})\s*-\s*([0-9]{3})\s*')
# 	match = regex.match(partno.upper())
# 	if match:
# 		return match.group(1) + "-" + match.group(2) + "-" + match.group(3)
# 	else:
# 		return ""
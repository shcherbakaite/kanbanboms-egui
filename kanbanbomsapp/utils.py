import re

def normalize_partno(partno):
	regex = re.compile('\s*([0-9]{5})\s*-\s*([a-zA-Z]{2})\s*-\s*([0-9]{3})\s*')
	match = regex.match(partno.upper())
	if match:
		return match.group(1) + "-" + match.group(2) + "-" + match.group(3)
	else:
		return ""
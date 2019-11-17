from django.core.management.base import BaseCommand
from kanbanbomsapp.models import Request, RequestEntry, BOM, BOMEntry

import csv
import re

from collections import defaultdict

from pprint import pprint

def normalize_partno(partno):
	regex = re.compile('\s*([0-9]{5})\s*-\s*([a-zA-Z]{2})\s*-\s*([0-9]{3})\s*')
	match = regex.match(partno.upper())
	if match:
		return match.group(1) + "-" + match.group(2) + "-" + match.group(3)
	else:
		return ""

def parse_bom_entry(csv_row):
	partno = normalize_partno(csv_row[9])
	if not partno:
		print("Not a valid part number(Sub-Assembly):" + csv_row[1] + " - " + csv_row[9] + " - " + csv_row[10])
		return ""
	else:
		return {
			'partno' : partno,
			'description' : csv_row[10],
			'quantity' : csv_row[6],
			'disabled' : True if csv_row[8].lower().replace(" ","") == 'yes' else False
		}

class Command(BaseCommand):
	args = '<foo bar ...>'
	help = 'our help string comes here'

	def add_arguments(self, parser):
		parser.add_argument('csvfile', nargs=1, type=str)

	def _populate_db(self, csvfile):
		print(csvfile)
		csvfile_handle = open(csvfile[0])
		csvreader = csv.reader(csvfile_handle)
		#boms = defaultdict(list)
		boms = dict()#(lambda:{'description' : "", 'parts' : []})
		for row in csvreader:
			partno = normalize_partno(row[1])
			if not partno:
				print("Not a valid part number(Assembly):" + row[1] + " - " + row[9] + " - " + row[10])
			else:
				#pprint(parse_bom_entry(row))
				# Assembly being encountered for the first time
				if partno not in boms:
					boms[partno] = {
						'description' : row[2],
						'parts' : []
					}
				entry = parse_bom_entry(row)
				if entry:
					boms[partno]['parts'].append(entry)

		#pprint(boms)
		for bom in boms.items():
			(bom_partno,entry) = bom

			b = BOM.objects.create(partno=bom_partno, description=entry['description'], batch_quantity=1)
			b.save();

			for part in entry['parts']:
				p = BOM.objects.filter(partno=part['partno'])
				if not p.exists():
					p = BOM.objects.create(partno=part['partno'], description=part['description'], batch_quantity=0)
					p.save()
				print(part['partno'])
				p = BOM.objects.get(partno=part['partno'])#.first()
				be = b.bomentry_set.create(bom=b,part=p,quantity=part['quantity'],disabled=part['disabled'])
			

	def handle(self, *args, **options):
		self._populate_db(options['csvfile'])
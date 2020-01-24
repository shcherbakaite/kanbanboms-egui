from django.db import models

class BOM(models.Model):
	partno = models.CharField(max_length=12)
	description = models.CharField(max_length=200)
	batch_quantity = models.IntegerField(default=0)
	def __str__(self):
		return self.partno + " " + self.description

class BOMEntry(models.Model):
	bom = models.ForeignKey(BOM, on_delete=models.CASCADE)
	part = models.ForeignKey(BOM, on_delete=models.CASCADE,  related_name='part')
	quantity = models.IntegerField(default=1)
	disabled = models.BooleanField(default=False)
	def __str__(self):
		return self.bom.partno + " -> " + self.part.partno + " x" + str(self.quantity)

class Request(models.Model):
	requested_by = models.CharField(max_length=25, default="")
	machine_number = models.CharField(max_length=25, default="")
	notes = models.CharField(max_length=200, default="")

class RequestEntry(models.Model):
	request = models.ForeignKey(Request, on_delete=models.CASCADE)
	part = models.ForeignKey(BOM, on_delete=models.CASCADE)
	quantity = models.IntegerField(default=0)

class TallyEntry(models.Model):
	badge = models.CharField(max_length=25)
	partno = models.CharField(max_length=25)
	cardno = models.CharField(max_length=25)
	timestamp = models.DateTimeField()

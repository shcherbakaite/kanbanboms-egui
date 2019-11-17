from django.shortcuts import render

from django.http import HttpResponse, HttpResponseRedirect

from .models import Request, RequestEntry, BOM, BOMEntry

from django.template import loader

from django.http import Http404

from django.urls import reverse

from django.db.models import Sum, F

from pprint import pprint

from collections import defaultdict

from functools import reduce

from .utils import normalize_partno

from django.http import JsonResponse

def edit_request(request, request_id, error="", partno=""):
	try:
		request_object = Request.objects.get(pk=request_id)
	except Request.DoesNotExist:
		raise Http404("Request does not exist")

	request_entry_objects = request_object.requestentry_set.order_by('part__partno').all()
	template = loader.get_template('kanbanbomsapp/edit_request.html')
	context = {
		'partno' : partno,
		'partno_validation_error' : error,
		'request' : request_object,
		'request_entries' : request_entry_objects
	}
	return HttpResponse(template.render(context, request))

def edit_request_add(request, request_id):
	request_object = Request.objects.get(pk=request_id)
	partno=normalize_partno(request.POST['partno'])
	try:
		part = BOM.objects.get(partno__iexact=partno)
	except BOM.DoesNotExist:
		return edit_request(request, request_id, error="Part number does not exist", partno=partno)
	request_entry_objects = request_object.requestentry_set.filter(part=part);
	if request_entry_objects.exists():
		request_entry_object = request_entry_objects.first()
		request_entry_object.quantity += part.batch_quantity
		request_entry_object.save()
	else:
		request_object.requestentry_set.create(part=part, quantity=part.batch_quantity)

	return HttpResponseRedirect(reverse('edit_request', args=(request_id,)))

def update_request(request, request_id):
	request_object = Request.objects.get(pk=request_id)
	for request_entry in request_object.requestentry_set.all():
		request_entry.quantity = request.POST[request_entry.part.partno]
		request_entry.save()
	return HttpResponseRedirect(reverse('edit_request', args=(request_id,)))

def print_request(request, request_id):
	request_object = Request.objects.get(pk=request_id)
	
	merged_bomsets = BOMEntry.objects.none()

	entries = request_object.requestentry_set.all()

	parts = []

	for request_entry in request_object.requestentry_set.all():
		for bom_entry in request_entry.part.bomentry_set.all():
			quantity = request_entry.quantity*bom_entry.quantity*(not bom_entry.disabled)
			if quantity:
				parts.append((bom_entry.part.partno, bom_entry.part.description, quantity));

	parts_groups = defaultdict(list)
	for part in parts:
		(partno,_,_) = part
		parts_groups[partno].append(part)

	aggregated_parts = []
	for part_group in parts_groups.items():
		(_, parts_list) = part_group
		aggregated_parts.append(reduce((lambda a, b: (a[0],a[1],a[2] + b[2])), parts_list))

	template = loader.get_template('kanbanbomsapp/print_request.html')
	context = {
		'request_id': request_id,
		'assemblies' : entries,
		'parts' : aggregated_parts,
	}
	return HttpResponse(template.render(context, request))

# WIP: Autocomplete feature
def edit_request_autocomplete(request):
	return JsonResponse([
		"75166-SD-100 Dashboard assembly, Roll/Slab, Full, Cup Holder, Switch", 
		"75167-SD-100 Stereo assembly, Roll/Slab, Full, Cup Holder, Switch"
		], safe=False)

def clear_request(request, request_id):
	request_object = Request.objects.get(pk=request_id)
	e = request_object.requestentry_set.all()
	e.delete()
	return HttpResponseRedirect(reverse('edit_request', args=(request_id,)))

def create_request(request):
	r = Request.objects.create()
	r.save()
	return HttpResponseRedirect(reverse('edit_request', args=(r.pk,)))

def index(request):
	return HttpResponseRedirect(reverse('create_request'))



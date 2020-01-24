from django.shortcuts import render

from django.http import HttpResponse, HttpResponseRedirect

from .models import Request, RequestEntry, BOM, BOMEntry

from .models import TallyEntry

from django.template import loader

from django.http import Http404

from django.urls import reverse

from django.db.models import Sum, F

from pprint import pprint

from collections import defaultdict

from functools import reduce

from .utils import normalize_partno, normalize_cardno

from django.http import JsonResponse

from io import BytesIO # barcodes

from django.db.models import F, Value

from django.db.models.functions import Concat

import barcode

import datetime

from datetime import datetime

import csv

def tally(request):
    template = loader.get_template('kanbanbomsapp/scanbadge.html')
    return HttpResponse(template.render({}, request))

def scan_badge(request):
    template = loader.get_template('kanbanbomsapp/scancard.html')
    context = {
      'badge' : request.POST['badge']
    }
    return HttpResponse(template.render(context, request))

def scan_card(request):
    template = loader.get_template('kanbanbomsapp/scancard.html')
    context = {
      'badge' : request.POST['badge']
    }
    tally = TallyEntry.objects.create(timestamp=datetime.now())
    tally.badge = request.POST['badge']
    tally.partno = normalize_partno(request.POST['partno'])
    tally.cardno = normalize_cardno(request.POST['partno'])
    tally.save()
    return HttpResponse(template.render(context, request))

def tally_csv(request):
    # Create the HttpResponse object with the appropriate CSV header.
    response = HttpResponse(content_type='text/csv')
    response['Content-Disposition'] = 'attachment; filename="kanbantally.csv"'

    writer = csv.writer(response)

    # writer.writerow(['Badge', 'Part Number', 'Card Number', 'Date', 'Time'])
    for entry in TallyEntry.objects.all():
        writer.writerow([entry.badge, entry.partno, entry.cardno, str(entry.timestamp.date()), str(entry.timestamp.time().strftime("%H:%M:%S"))])

    return response

def update_info(request, request_id):
    request_object = Request.objects.get(pk=request_id)
    request_object.requested_by = request.POST['requestedby'];
    request_object.notes = request.POST['notes'];
    request_object.machine_number = request.POST['machine'];
    request_object.save()
    return HttpResponse(request.POST['machine'])

def make_card(request):
    template = loader.get_template('kanbanbomsapp/cardform.html')

    # template = loader.get_template('kanbanbomsapp/card.html')
    
    # cards = []
    # if request.POST['partno1']:
    #     cards.append({
    #         'partno' : request.POST['partno1'],
    #         'description' : request.POST['description1'],
    #         'batch' : request.POST['batch1'],
    #     })
    # # if request.POST['partno2']:
    #     cards.append({
    #         'partno' : request.POST['partno2'],
    #         'description' : request.POST['description2'],
    #         'batch' : request.POST['batch2'],
    #     })
    # if request.POST['partno3']:
    #     cards.append({
    #         'partno' : request.POST['partno3'],
    #         'description' : request.POST['description3'],
    #         'batch' : request.POST['batch3'],
    #     })
    # if request.POST['partno4']:
    #     cards.append({
    #         'partno' : request.POST['partno4'],
    #         'description' : request.POST['description4'],
    #         'batch' : request.POST['batch4'],
    #     })


    context = {
        'partno' : request.POST.get('partno1', "99999-XX-999"),
        'description' : request.POST.get('description1', "Part Description"),
        'batch' : request.POST.get('batch1', "1")
    }
    
    return HttpResponse(template.render(context, request))

def render_card(request):
    template = loader.get_template('kanbanbomsapp/card.html')
    
    # cards = []
    # if request.POST['partno1']:
    #     cards.append({
    #         'partno' : request.POST['partno1'],
    #         'description' : request.POST['description1'],
    #         'batch' : request.POST['batch1'],
    #     })
    # # if request.POST['partno2']:
    #     cards.append({
    #         'partno' : request.POST['partno2'],
    #         'description' : request.POST['description2'],
    #         'batch' : request.POST['batch2'],
    #     })
    # if request.POST['partno3']:
    #     cards.append({
    #         'partno' : request.POST['partno3'],
    #         'description' : request.POST['description3'],
    #         'batch' : request.POST['batch3'],
    #     })
    # if request.POST['partno4']:
    #     cards.append({
    #         'partno' : request.POST['partno4'],
    #         'description' : request.POST['description4'],
    #         'batch' : request.POST['batch4'],
    #     })
    context = {
        'partno' : request.GET['p'],
        'description' : request.GET['d'],
        'batch' : request.GET['b'],
    }
    return HttpResponse(template.render(context, request))

def make_barcode(request, text):
    ean = barcode.codex.Code39(text,add_checksum=False)
    ean.default_writer_options['write_text'] = False
    b = BytesIO()
    ean.write(b)
    return HttpResponse(b.getvalue(), content_type="image/svg+xml")

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

    # Sort by part number
    aggregated_parts = sorted(aggregated_parts, key=lambda x: x[0])

    template = loader.get_template('kanbanbomsapp/print_request.html')
    context = {
        'request_id': request_id,
        'request' : request_object,
        'assemblies' : entries,
        'parts' : aggregated_parts,
        'date' : datetime.today().strftime('%m/%d/%Y')
    }
    return HttpResponse(template.render(context, request))

# WIP: Autocomplete feature
def edit_request_search(request):
    template = loader.get_template('kanbanbomsapp/search_response.html')
    response_entries = []

    keywords = request.GET['f'].split()

    if len(keywords) == 0:
        return HttpResponse("Type in a query...")

    response_entries = BOM.objects.all().annotate(text=Concat(F("partno"), F("description")))

    for keyword in keywords:
        response_entries = response_entries.filter(text__icontains=keyword);

    context = {
        'response_entries' : response_entries
    }

    if len(response_entries) == 0:
        return HttpResponse("No results...")

    return HttpResponse(template.render(context, request))

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



from django.urls import path

from . import views

from django.conf.urls.static import static

from django.conf import settings

urlpatterns = [
	path('', views.index, name='index'),
	path('editrequest/updateinfo/<int:request_id>', views.update_info, name='update_info'),
	path('tally', views.tally, name='tally'),
	path('tallycsv', views.tally_csv, name='tally_csv'),
	path('scanbadge', views.scan_badge, name='scan_badge'),
	path('edit_location/<int:request_id>', views.edit_location, name='edit_location'),
	path('makecard', views.make_card, name='make_card'),
	path('scancard', views.scan_card, name='scan_card'),
	path('rendercard', views.render_card, name='render_card'),
	path('makebarcode/<str:text>', views.make_barcode, name='make_barcode'),
	path('createrequest', views.create_request, name='create_request'),
	path('editrequest/<int:request_id>', views.edit_request, name='edit_request'),
    path('editrequest/search', views.edit_request_search, name='edit_request_search'), # WIP: Autocomplete feature
    path('editrequest/<int:request_id>/add', views.edit_request_add, name='edit_request_add'),
    path('clearrequest/<int:request_id>', views.clear_request, name='clear_request'),
    path('printrequest/<int:request_id>', views.print_request, name='print_request'),
    path('updaterequest/<int:request_id>', views.update_request, name='update_request'),
]
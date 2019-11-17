from django.urls import path

from . import views

urlpatterns = [
	path('', views.index, name='index'),
	path('createrequest', views.create_request, name='create_request'),
	path('editrequest/<int:request_id>', views.edit_request, name='edit_request'),
    path('editrequest/autocomplete', views.edit_request_autocomplete, name='edit_request_autocomplete'), # WIP: Autocomplete feature
    path('editrequest/<int:request_id>/add', views.edit_request_add, name='edit_request_add'),
    path('clearrequest/<int:request_id>', views.clear_request, name='clear_request'),
    path('printrequest/<int:request_id>', views.print_request, name='print_request'),
    path('updaterequest/<int:request_id>', views.update_request, name='update_request'),
]
from django.contrib import admin

from .models import BOM, BOMEntry, Request, RequestEntry

class BOMEntryInline(admin.TabularInline):
	model = BOMEntry
	extra = 1
	fk_name = "bom"
	fields = ('part','quantity')
	autocomplete_fields = ("part",)

class BOMAdmin(admin.ModelAdmin):
	search_fields = ("partno","description")
	inlines = [
		BOMEntryInline,
	]
	class Media:
		css = {
			'all': ('css/custom_admin.css', )     # Include extra css to hide titles over each entry
		}

admin.site.register(BOM, BOMAdmin)
#admin.site.register(BOMEntry)
admin.site.register(Request)
admin.site.register(RequestEntry)
#!/bin/bash
nginx
uwsgi -s :8001 --master --processes 10 --module kanbanboms.wsgi
 
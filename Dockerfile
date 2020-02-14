# The first instruction is what image we want to base our container on
# We Use an official Python runtime as a parent image
FROM python:3.6

# The enviroment variable ensures that the python output is set straight
# to the terminal with out buffering it first
ENV PYTHONUNBUFFERED 1

# create root directory for our project in the container
RUN mkdir /kanbanboms_service

# Set the working directory to /kanbanboms_service
WORKDIR /kanbanboms_service

# Copy the current directory contents into the container at /kanbanboms_service
ADD . /kanbanboms_service/

# Install any needed packages specified in requirements.txt
RUN pip install -r requirements.txt

RUN apt-get -y update

RUN apt-get -y install apt-utils

RUN apt-get -y install nginx

RUN cp ./kanbanboms_nginx.conf /etc/nginx/sites-enabled

RUN chmod u+x ./start_server.sh

RUN rm db.sqlite3

RUN ln -s /var/data/db.sqlite3 .

#CMD bash

CMD ./start_server.sh

#CMD python manage.py runserver 0.0.0.0:8010
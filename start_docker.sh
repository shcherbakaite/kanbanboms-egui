№docker container run  --mount type=bind,source=/var/kanbanboms/,target=/var/data -it --publish 8000:8000 kanbanbomimg 

# Overwrite sources with development files
#docker container run  --mount type=bind,source=/var/kanbanboms/,target=/var/data --mount type=bind,source=/home/vlsh/Sources/kanbanboms,target=/kanbanboms_service/ -it --publish 8000:8000 kanbanbomimg 

# Interactive shell
docker container run  --mount type=bind,source=/var/kanbanboms/,target=/var/data --mount type=bind,source=/home/vlsh/Sources/kanbanboms,target=/kanbanboms_service/ -it --entrypoint bash --publish 8000:8000 kanbanbomimg 


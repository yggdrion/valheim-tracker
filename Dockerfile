FROM python:3.12-slim
WORKDIR /app
COPY *.py dashboard.html achievements.json stat_map.json stat_names.json names.json ./
COPY vendor vendor
COPY fonts fonts
ENV PYTHONUNBUFFERED=1
EXPOSE 8080
CMD ["python", "server.py", "--port", "8080", "--state", "/app/state"]

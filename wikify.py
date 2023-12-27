import concurrent.futures
import os
import time
from typing import Callable
from urllib.parse import unquote

import markdown
from bs4 import BeautifulSoup
from jinja2 import Template
from lxml import html

custom_template = """<!DOCTYPE html>
<html lang="ko">
<head>
    <link href="https://fonts.googleapis.com" rel="preconnect"/>
    <link crossorigin="" href="https://fonts.gstatic.com" rel="preconnect"/>
    <link href="https://fonts.googleapis.com/css2?family=Noto+Sans+KR:wght@100;200;300;400;500;600;700;800;900&amp;display=swap"
          rel="stylesheet"/>
    <meta charset="utf-8"/>
    <link rel="stylesheet" href="./assets/styles/atom-one-dark.css">
    <script src="./assets/styles/highlight.min.js"></script>
    <script>
        document.addEventListener('DOMContentLoaded', (event) => {
            document.querySelectorAll('pre code').forEach((block) => {
                console.log(block)
                hljs.highlightElement(block);
            });
        });
    </script>
    <style>
        body {
            font-family: 'Noto Sans KR', sans-serif;
            background-color: #222233;
            color: #e3e3e3;
            margin: 0;
            padding: 1rem 10rem;
            font-size: 1.2rem;
        }

        img {
            max-width: 100%;
            height: auto;
            display: block;
            margin: 0 auto;
        }

        h1 {
            font-size: 3rem;
        }

        h2 {
            font-size: 2.8rem;
        }

        h3 {
            font-size: 2.6rem;
        }

        h4 {
            font-size: 2.4rem;
        }

        h5 {
            font-size: 2.2rem;
        }

        h6 {
            font-size: 2rem;
        }

        a {
            color: #ff8c00;
            text-decoration: none;
        }

        a:visited {
            color: #ffa07a;
        }

        blockquote {
            background-color: #f9f9f9;
            border-left: 6px solid #0077cc;
            padding: 1rem 1.5rem;
            margin: 10px 0;
            color: #555;
        }

        code {
            font-size: 1.5rem;
        }

    </style>
    <title>{{ title }}</title>
</head>
<body>{{ body }}</body>
</html>"""


def markdown_to_html(markdown_text) -> str:
    html_content = markdown.markdown(markdown_text,
                                     extensions=['tables', 'markdown_checklist.extension', 'fenced_code'])
    html_element = html.fromstring(html_content)

    return html.tostring(html_element, pretty_print=True, encoding='unicode')


def read_file(filepath: str) -> str:
    with open(filepath, 'r') as file:
        return file.read()


def save_file(filepath: str, content: str) -> None:
    with open(filepath, 'w') as file:
        file.write(content)
        file.flush()


def scan_dir(directory_path) -> list:
    filenames = [f for f in os.listdir(directory_path) if os.path.isfile(os.path.join(directory_path, f))]
    return list(filter(lambda x: x.endswith('.md'), filenames))


def traverse_in_parallel(directory_path: str, handler: Callable[[str], None]):
    filenames = scan_dir(directory_path)
    targets = list(map(lambda x: os.path.join(directory_path, x), filenames))
    with concurrent.futures.ProcessPoolExecutor() as executor:
        executor.map(handler, targets)


def change_file_extension(file_path, new_extension):
    root, old_extension = os.path.splitext(file_path)
    if (new_extension == ''):
        new_file_path = root
    else:
        new_file_path = f"{root}.{new_extension}"
    return new_file_path


def extract_filename(file_path):
    return os.path.basename(file_path)


def extract_filename_without_extension(file_path):
    filename = extract_filename(file_path)
    filename_without_ext, _ = os.path.splitext(filename)
    return filename_without_ext


def post_process(html_document: str) -> str:
    soup = BeautifulSoup(html_document, 'html.parser')
    for a_tag in soup.find_all('a', href=True):
        origin = a_tag['href']
        if (origin.startswith('http')):
            continue
        if (origin.startswith('%')):
            origin = unquote(origin)
        if origin[0].isalpha() or origin[0] == '.' or origin[0].isdigit() or (ord('가') <= ord(origin[0]) <= ord('힣')):
            a_tag['href'] = f"{origin}.html"
    return str(soup)


def wiki_handler(filepath: str) -> None:
    origin_content = read_file(filepath)
    html_output = markdown_to_html(origin_content)
    document_name = extract_filename_without_extension(filepath)
    html_filepath = change_file_extension(filepath, 'html')
    html_filename = extract_filename(html_filepath)
    html_output_path = os.path.join('./docs', html_filename)
    rendered_document = render_template(document_name, html_output)
    post_processed = post_process(rendered_document)

    save_file(html_output_path, post_processed)


def render_template(document_name: str, html_body: str) -> str:
    template = Template(custom_template)
    return template.render(title=document_name, body=html_body)


if __name__ == '__main__':
    start_time = time.perf_counter()
    traverse_in_parallel('./wiki', wiki_handler)
    end_time = time.perf_counter()

    elapsed_time = end_time - start_time
    print(f"Elapsed Time: {elapsed_time} seconds")

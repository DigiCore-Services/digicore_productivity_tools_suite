README Code of conduct Contributing Apache-2.0 license      Security

Advanced Usage

Multipart Uploads

   gws drive files create --json ' {"name": "report. pdf"}' --upload . / report . pdf

Pagination

| Flag Description |  | Default | 
|---|---|---|

| --page-all | Auto-paginate, one JSON line per page (NDJSON) | off | 

| --page-limit <N> | Max pages to fetch | 10 | 

| --page-delay <MS> | Delay between pages | 100 ms | 

Google Sheets — Shell Escaping

Sheets ranges use ! which bash interprets as history expansion. Always wrap values in single quotes:
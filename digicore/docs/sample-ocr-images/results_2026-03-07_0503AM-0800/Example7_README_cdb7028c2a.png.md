https://github.com/googleworkspace/cli?tab=readme-ov-file#why-gws

  Monica - Your Cha... The Top 10 Skills E... CAREER-and-JOBS Braina Ü FAVORITEZ Ü ADM Ü Smart HOME

 README Code of conduct AR Contributing Apache-2.0 license security

Why gws?

For humans — stop writing curl calls against REST docs. gws gives you --help on every resource, --dry-
run  to preview requests, and auto-pagination.

For AI agents — every response is structured JSON. Pair it with the included agent skills and your LLM can
manage Workspace without custom tooling.

  #  List the 10 most recent files

  gws drive files list --params ' {"pageSize": 10}

  #  Create a spreadsheet
  gws sheets spreadsheets create --json ' {"properties" : {"title" : "QI Budget"}} '

  #  Send a Chat message

  gws chat spaces messages create \
     --params ' {"parent": "spaces/xyz"}' \
     --json ' {"text" : "Deploy complete. "}' \
     --dry-run

  #  Introspect any method's request/ response schema

  gws schema drive. files. list

  #  Stream paginated results as NDJSON
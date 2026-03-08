Browser-assisted auth (human or agent)

| You | can complete OAuth either manually or with browser automation. |  |  | 
|---|---|---|---|
| ' | Human | flow: run gws auth login , open the printed URL, approve scopes. |  | 
| ' | Agent-assisted flow: the agent opens the URL, selects account, handles consent prompts, and returns |  |  | 
|  | control | once the localhost callback succeeds. |  | 
| If consent |  | shows "Google hasn't verified this app" (testing mode), click Continue. If scope checkboxes |  | 
| appear, select |  |  | required scopes (or Select all) before continuing. | 
| Headless |  | / | CI (export flow) | 
| 1. | Complete |  | interactive auth on a machine with a browser. | 
| 2. | Export | credentials: |  | 
| gws |  | auth export --unmasked > credentials . j son |  | 

| 3. | On the | headless machine: |  | 
       export FILE=/path/to/credentia1s.json
       gws drive files list # just works
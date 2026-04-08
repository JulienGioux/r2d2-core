console.log("Starting test...");
import { queryNotebookLM } from "./index.js";
queryNotebookLM("https://notebooklm.google.com/notebook/4dd65131-ea87-47a3-8958-a647351c4050", "Que doit-on supprimer exactement dans les mocks de r2d2-cortex ?")
  .then(res => console.log("RÉPONSE DE RUSTYMASTER:\n" + res))
  .catch(err => console.error(err));

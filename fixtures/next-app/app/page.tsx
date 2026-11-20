import Link from "next/link";
import { Modal } from "../components/modal";
import { Tabs } from "../components/tabs";
import { Toast } from "../components/toast";

const COURSES = ["Matemática", "Português", "História"];

export default function Home() {
  return (
    <main>
      <h1>Cursos</h1>
      <ul>
        {COURSES.map((c) => (
          <li key={c}>{c}</li>
        ))}
      </ul>
      <div className="bar" role="progressbar" aria-valuenow={60}>
        <div style={{ width: "60%" }} />
      </div>
      <details>
        <summary>Como funciona?</summary>
        <p>Assista às aulas e faça as questões.</p>
      </details>
      <Tabs />
      <Modal />
      <Toast />
      <button className="btn fancy">Já animado</button>
      <Link href="/sobre" className="btn">
        Ver mais
      </Link>
    </main>
  );
}

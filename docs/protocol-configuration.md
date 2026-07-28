# Configuración Global del Protocolo (Protocol Configuration)

El contrato inteligente incluye un módulo de configuración global, diseñado para manejar parámetros que afectan el comportamiento a nivel de protocolo (ej. `min_transfer_amount`, `max_batch_size`).

## Separación de Preocupaciones (Separation of Concerns)

Para preservar la escalabilidad, la modularidad y el rendimiento, la configuración del protocolo no consolida *todos* los estados globales (como `Paused`, `SupplyCap`, o `AssetStatus`) en una sola estructura. Almacenar variables que cambian independientemente y tienen diferentes modelos de seguridad (ej. pausa de emergencia frente a tokenomics) bajo diferentes claves de datos (`DataKey`) asegura:
1. **Bajos Costes de Gas:** Las comprobaciones de alto rendimiento (como verificar si el contrato está pausado en cada transferencia) no requieren la deserialización de todo el conjunto de configuraciones.
2. **Seguridad Escalable:** Los parámetros tienen barreras y responsables lógicos distintos.

La estructura `ProtocolConfig` sirve para centralizar parámetros RWA flexibles.

```rust
pub struct ProtocolConfig {
    pub min_transfer_amount: i128,
    pub max_batch_size: u32,
}
```

## Gobernanza de 2 Pasos (2-Step Governance)

Para evitar bloqueos accidentales (bricking) debido a errores de dedo o datos malformados, cualquier actualización de configuración pasa por un proceso de 2 pasos, exclusivo para el **Administrador Supremo (Supreme Admin)**.

1. **Proponer (`propose_config`)**: El administrador envía un nuevo estado de configuración. Este estado es validado (ej. cantidades negativas, tamaños de lote que romperían los límites computacionales). Si pasa, se almacena como `ProtocolConfigCandidate`.
2. **Aceptar (`accept_config`)**: El administrador aprueba la configuración pendiente, moviéndola de `Candidate` a activa.
3. **Cancelar (`cancel_config_proposal`)**: En caso de error o cambio de opinión, el candidato pendiente puede ser eliminado.

## Suposiciones de Seguridad y RWA Guardrails

- **Límites Estrictos de Validación:** `propose_config` implementa aserciones rígidas (ej. `max_batch_size == 0` es rechazado). Las configuraciones que puedan bloquear operaciones en el contrato están prohibidas.
- **Pausa de Emergencia:** Al igual que el resto del protocolo, las actualizaciones de configuración están bloqueadas cuando el contrato se encuentra pausado.
- **Sin Asesoramiento Legal:** Este módulo implementa reglas a nivel de código. No constituye asesoría financiera ni de cumplimiento regulatorio local.

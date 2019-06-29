# rust-prometheus-nvidia-gpu-exporter
NVIDIA GPU Prometheus Exporter written in Rust


This is a [Prometheus Exporter](https://prometheus.io/docs/instrumenting/exporters/) for exporting NVIDIA GPU metrics. 
It uses the [Rust bindings](https://github.com/Cldfire/nvml-wrapper) for [NVIDIA Management Library](https://developer.nvidia.com/nvidia-management-library-nvml) 
(NVML) which is a C-based API that can be used for monitoring NVIDIA GPU devices. Unlike some other similar exporters, 
it does not call the [`nvidia-smi`](https://developer.nvidia.com/nvidia-system-management-interface) binary.



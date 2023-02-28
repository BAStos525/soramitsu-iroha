@Library('jenkins-library@feature/DOPS-2261/iroha2-pr-deploy') _

new org.iroha2.MainPipeline().call(
    k8sPrDeploy: true,
    vaultPrPath: "argocd-cc/src/charts/iroha2/environments/tachi/",
    vaultUser: "iroha2-ro",
    vaultCredId: "iroha2VaultCreds",
    valuesDestPath: "argocd-cc/src/charts/iroha2/",
    devValuesPath: "dev/test/"
)
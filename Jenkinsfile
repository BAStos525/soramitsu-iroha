@Library('jenkins-library@feature/DOPS-2261/iroha2-pr-deploy') _

def pipeline = new org.iroha2.MainPipeline(steps: this,
    k8sPrDeploy: true,
    vaultPrPath: "argocd-cc/src/charts/sora2/polkaswap-exchange-web/environments/tachi/",
    vaultUser: "iroha2-ro",
    vaultCredId: "iroha2VaultCreds",
    valuesDestPath: "argocd-cc/src/charts/iroha2/",
    devValuesPath: "dev/test/"
)
pipeline.runPipeline()